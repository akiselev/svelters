#!/usr/bin/env node
// Compares two op logs (plans/architecture/backend-abi.md "Op-log format") by the native state
// they produce, not by their text: each log is replayed into a tree, and the states are compared
// before every `event` / `sync` / `deferred` / `gone` / `changed` line and at the end. A state is: live
// handles with kind, props, children per slot (in order), child attributes (ABI v2.2 `attr`, cleared when
// the child is removed), listened signals with their phase word, observed props (ABI v2.6 `observe`),
// the methods called on them, and
// the values JS returned for `sync` lines so far (`result` lines), and the widget name (`style #h
// name`, kept as the prop `(name)`; other `style` lines are not compared), and the observed native
// state a trace writer recorded (WG3 `state #h {...}` lines: each one is a boundary of its own).
//
// Usage: node tools/oplog-compare.mjs <a.oplog> <b.oplog> [--default prop=<json>]... [--states]
//   --default label=""   a prop set to this value counts as never set (e.g. a label written "")
//   --states             compare only the `state` lines, in order, with handles matched by first
//                        appearance: two toolkits' traces of one pin (GTK `entry` vs egui `text-edit`)
// Exit status 0 when every boundary state is equal, 1 otherwise.
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

function parseLine(line) {
  const sp = (s, n) => {
    const out = [];
    let rest = s;
    for (let i = 0; i < n - 1; i++) {
      const j = rest.indexOf(' ');
      if (j < 0) break;
      out.push(rest.slice(0, j));
      rest = rest.slice(j + 1);
    }
    out.push(rest);
    return out;
  };
  const op = line.slice(0, line.indexOf(' '));
  const h = (s) => Number(s.replace(/^#/, ''));
  switch (op) {
    case 'create': {
      const [, kind, n] = sp(line, 3);
      return { op, kind, h: h(n) };
    }
    case 'insert': {
      const [, p, c, before, slot] = sp(line, 5);
      const b = before.slice('before='.length);
      return { op, p: h(p), c: h(c), before: b === 'end' ? null : h(b), slot: slot.slice('slot='.length) };
    }
    case 'remove': {
      const [, p, c] = sp(line, 3);
      return { op, p: h(p), c: h(c) };
    }
    case 'destroy':
      return { op, h: h(sp(line, 2)[1]) };
    case 'construct': {
      // ABI v2.5: `construct #h Kind {props}`
      const [, n, kind, json] = sp(line, 4);
      return { op, h: h(n), kind, props: JSON.parse(json) };
    }
    case 'static': {
      // ABI v2.5: `static Kind.method [args] [#h]`; a constructor-shaped name ends with the new handle
      const m = /^static (\S+) (\[.*\])(?: #(\d+))?$/.exec(line);
      if (!m) throw new Error(`bad static op: ${line}`);
      return { op, call: m[1], args: JSON.parse(m[2]), h: m[3] ? Number(m[3]) : null };
    }
    case 'set': {
      const [, n, prop, json] = sp(line, 4);
      return { op, h: h(n), prop, value: JSON.parse(json) };
    }
    case 'call': {
      const [, n, method, json] = sp(line, 4);
      return { op, h: h(n), method, args: JSON.parse(json) };
    }
    case 'listen': {
      const [, n, signal, rest] = sp(line, 4);
      const [on, flags] = rest.split(' ');
      return { op, h: h(n), signal: flags ? `${signal} ${flags}` : signal, on: on === 'on' };
    }
    case 'observe': {
      // ABI v2.6: `observe #h prop on|off`
      const [, n, prop, on] = sp(line, 4);
      return { op, h: h(n), prop, on: on === 'on' };
    }
    case 'changed': {
      // ABI v2.6: `changed #h prop [json]` (a scripted native change, with the new value when it has one)
      const [, n, prop, json] = sp(line, 4);
      return { op, h: h(n), prop, value: json === undefined || json === '' ? undefined : JSON.parse(json) };
    }
    case 'event':
    case 'sync':
    case 'deferred': {
      // `event #h sig [args] [flags word]`
      const [, n, signal, rest] = sp(line, 4);
      const m = /^(.*\])(?: (\S+))?$/.exec(rest);
      return { op, h: h(n), signal, args: JSON.parse(m ? m[1] : rest), flags: m?.[2] ?? '' };
    }
    case 'result': {
      const [, n, json] = sp(line, 3);
      return { op, h: h(n), value: JSON.parse(json) };
    }
    case 'attr': {
      const [, p, c, name, json] = sp(line, 5);
      return { op, p: h(p), c: h(c), name, value: JSON.parse(json) };
    }
    case 'state': {
      // WG3 trace writer: `state #h {"text":…,"sel":[a,f],"caret":f,"focus":b}` at a checkpoint
      const [, n, json] = sp(line, 3);
      return { op, h: h(n), value: JSON.parse(json) };
    }
    case 'gone': {
      const [, n, reason] = sp(line, 3);
      return { op, h: h(n), reason };
    }
    case 'style': {
      // X6 styling: the widget name (`id`) is state; classes (scope hashes, kind classes), inline
      // CSS text, sheets and the scheme differ in spelling between pipelines and are not compared
      const [, n, what, json] = sp(line, 4);
      if (what === 'name' && n.startsWith('#')) return { op: 'name', h: h(n), value: JSON.parse(json) };
      return { op: 'ignore' };
    }
    default:
      throw new Error(`unknown op: ${line}`);
  }
}

/** `default-width` -> `defaultWidth` (hosts resolve both spellings of a GTK property). */
const camel = (name) => name.replace(/-([a-z0-9])/g, (_, c) => c.toUpperCase());

/** Replays `text` (`camelProps`: compare prop names in camelCase); returns { boundaries: [{ line, state }], final, ops } (state = canonical JSON). */
export function replay(text, { defaults = {}, camelProps = false } = {}) {
  const nodes = new Map(); // h -> { kind, props, parent, slots: Map, listening: Set, calls: [], attrs: Map }
  const get = (h, line) => {
    const n = nodes.get(h);
    if (!n) throw new Error(`op on unknown or dead handle #${h}: ${line}`);
    return n;
  };
  const unlink = (h) => {
    const n = nodes.get(h);
    if (!n || n.parent == null) return;
    const p = nodes.get(n.parent.h);
    if (p) {
      const list = p.slots.get(n.parent.slot);
      list.splice(list.indexOf(h), 1);
      if (!list.length) p.slots.delete(n.parent.slot);
    }
    n.parent = null;
    n.attrs.clear();
  };
  const kill = (h) => {
    unlink(h);
    nodes.delete(h);
  };
  const snapshot = () => {
    const out = [...nodes.keys()].sort((a, b) => a - b).map((h) => {
      const n = nodes.get(h);
      const props = Object.fromEntries(
        [...n.props]
          // a null write resets the prop to the native default: the same as never set
          .filter(([k, v]) => v !== null && !(k in defaults && Object.is(defaults[k], v)))
          .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)),
      );
      const slots = Object.fromEntries([...n.slots].sort(([a], [b]) => (a < b ? -1 : 1)));
      const node = { h, kind: n.kind, props, slots, listening: [...n.listening].sort(), calls: n.calls };
      if (n.observing?.size) node.observing = [...n.observing].sort();
      if (n.attrs.size) node.attrs = Object.fromEntries([...n.attrs].sort(([a], [b]) => (a < b ? -1 : 1)));
      if (n.state !== undefined) node.state = n.state;
      return node;
    });
    if (results.length) out.push({ h: 'results', results });
    if (statics.length) out.push({ h: 'statics', statics });
    return JSON.stringify(out);
  };

  const boundaries = [];
  const results = []; // sync results, in order
  const statics = []; // static calls (ABI v2.5), in order
  const lines = text.split('\n').filter(Boolean);
  for (const line of lines) {
    const o = parseLine(line);
    switch (o.op) {
      case 'create':
        if (nodes.has(o.h)) throw new Error(`handle reused: ${line}`);
        nodes.set(o.h, { kind: o.kind, props: new Map(), parent: null, slots: new Map(), listening: new Set(), calls: [], attrs: new Map() });
        break;
      case 'insert': {
        const p = get(o.p, line);
        const c = get(o.c, line);
        if (c.parent) throw new Error(`insert of an inserted child: ${line}`);
        const list = p.slots.get(o.slot) ?? [];
        p.slots.set(o.slot, list);
        let i = list.length;
        if (o.before != null) {
          i = list.indexOf(o.before);
          if (i < 0) throw new Error(`before is not a child in that slot: ${line}`);
        }
        list.splice(i, 0, o.c);
        c.parent = { h: o.p, slot: o.slot };
        break;
      }
      case 'remove': {
        const c = get(o.c, line);
        if (!c.parent || c.parent.h !== o.p) throw new Error(`remove of a non-child: ${line}`);
        unlink(o.c);
        break;
      }
      case 'destroy':
        get(o.h, line);
        for (const [, list] of nodes.get(o.h).slots) for (const c of [...list]) nodes.get(c).parent = null;
        kill(o.h);
        break;
      case 'set':
        get(o.h, line).props.set(camelProps ? camel(o.prop) : o.prop, o.value);
        break;
      case 'construct':
        if (nodes.has(o.h)) throw new Error(`handle reused: ${line}`);
        nodes.set(o.h, {
          kind: o.kind,
          props: new Map(Object.entries(o.props).map(([k, v]) => [camelProps ? camel(k) : k, v])),
          parent: null,
          slots: new Map(),
          listening: new Set(),
          calls: [],
          attrs: new Map(),
        });
        break;
      case 'static':
        statics.push(`${o.call} ${JSON.stringify(o.args)}`);
        if (o.h !== null) {
          nodes.set(o.h, { kind: o.call.slice(0, o.call.lastIndexOf('.')), props: new Map(), parent: null, slots: new Map(), listening: new Set(), calls: [], attrs: new Map() });
        }
        break;
      case 'call':
        get(o.h, line).calls.push(`${o.method} ${JSON.stringify(o.args)}`);
        break;
      case 'listen':
        if (o.on) get(o.h, line).listening.add(o.signal);
        else get(o.h, line).listening.delete(o.signal);
        break;
      case 'observe': {
        const n = get(o.h, line);
        n.observing ??= new Set();
        if (o.on) n.observing.add(camelProps ? camel(o.prop) : o.prop);
        else n.observing.delete(camelProps ? camel(o.prop) : o.prop);
        break;
      }
      case 'attr': {
        const c = get(o.c, line);
        if (!c.parent || c.parent.h !== o.p) throw new Error(`attr on a non-child: ${line}`);
        if (o.value === null) c.attrs.delete(o.name);
        else c.attrs.set(o.name, o.value);
        break;
      }
      case 'result':
        // the value JS returned to the toolkit for the preceding `sync`: part of the next state
        results.push(`${o.h} ${JSON.stringify(o.value)}`);
        break;
      case 'ignore':
        break;
      case 'state':
        get(o.h, line).state = canonical(o.value);
        boundaries.push({ line: `state #${o.h}`, state: snapshot() });
        break;
      case 'name': {
        const n = get(o.h, line);
        if (o.value === null) n.props.delete('(name)');
        else n.props.set('(name)', o.value);
        break;
      }
      case 'event':
      case 'sync':
      case 'deferred':
      case 'changed':
      case 'gone':
        boundaries.push({ line, state: snapshot() });
        if (o.op === 'gone') kill(o.h);
        break;
    }
  }
  return { boundaries, final: snapshot(), ops: lines.filter((l) => !/^(event|sync|deferred|result|gone|changed|state) /.test(l)).length };
}

/** A JSON value with object keys sorted (state payloads compare by content, not key order). */
function canonical(v) {
  if (Array.isArray(v)) return v.map(canonical);
  if (v && typeof v === 'object') return Object.fromEntries(Object.keys(v).sort().map((k) => [k, canonical(v[k])]));
  return v;
}

/** The `state` lines of a trace, in order, with handles renumbered by first appearance. */
export function states(text) {
  const ids = new Map();
  const out = [];
  for (const line of text.split('\n')) {
    if (!line.startsWith('state ')) continue;
    const o = parseLine(line);
    if (!ids.has(o.h)) ids.set(o.h, ids.size + 1);
    out.push({ n: ids.get(o.h), json: JSON.stringify(canonical(o.value)) });
  }
  return out;
}

/** `--states`: compares two traces by their `state` lines only. */
export function compareStates(a, b) {
  const x = states(a);
  const y = states(b);
  const problems = [];
  if (x.length !== y.length) problems.push(`a has ${x.length} state lines, b has ${y.length}`);
  for (let i = 0; i < Math.min(x.length, y.length); i++) {
    if (x[i].n !== y[i].n) problems.push(`state line ${i}: node ${x[i].n} in a, ${y[i].n} in b`);
    else if (x[i].json !== y[i].json) problems.push(`state line ${i} (node ${x[i].n}):\n    a: ${x[i].json}\n    b: ${y[i].json}`);
  }
  return { equal: problems.length === 0, problems, states: x.length };
}

function firstDiff(a, b) {
  const x = JSON.parse(a);
  const y = JSON.parse(b);
  const bx = new Map(x.map((n) => [n.h ?? 'results', n]));
  const by = new Map(y.map((n) => [n.h ?? 'results', n]));
  for (const h of new Set([...bx.keys(), ...by.keys()])) {
    const p = JSON.stringify(bx.get(h) ?? null);
    const q = JSON.stringify(by.get(h) ?? null);
    if (p !== q) return `#${h}\n    a: ${p}\n    b: ${q}`;
  }
  return '(no node differs)';
}

/** Compares two logs; returns { equal, problems: string[], opsA, opsB, boundaries }. */
export function compare(a, b, opts = {}) {
  const ra = replay(a, opts);
  const rb = replay(b, opts);
  const problems = [];
  const n = Math.max(ra.boundaries.length, rb.boundaries.length);
  for (let i = 0; i < n; i++) {
    const x = ra.boundaries[i];
    const y = rb.boundaries[i];
    if (!x || !y || x.line !== y.line) {
      problems.push(`boundary ${i}: a has "${x?.line}", b has "${y?.line}"`);
      break;
    }
    if (x.state !== y.state) problems.push(`state before "${x.line}" (boundary ${i}) differs at ${firstDiff(x.state, y.state)}`);
  }
  if (ra.final !== rb.final) problems.push(`final state differs at ${firstDiff(ra.final, rb.final)}`);
  return { equal: problems.length === 0, problems, opsA: ra.ops, opsB: rb.ops, boundaries: n };
}

export function parseDefaults(args) {
  const defaults = {};
  for (let i = 0; i < args.length; i++) {
    if (args[i] !== '--default') continue;
    const [k, ...v] = args[++i].split('=');
    defaults[k] = JSON.parse(v.join('='));
  }
  return defaults;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === fs.realpathSync(process.argv[1])) {
  const args = process.argv.slice(2);
  const files = args.filter((x, i) => !x.startsWith('--') && args[i - 1] !== '--default');
  if (files.length !== 2) {
    console.error('usage: oplog-compare.mjs <a.oplog> <b.oplog> [--default prop=<json>]... [--states]');
    process.exit(2);
  }
  if (args.includes('--states')) {
    const r = compareStates(fs.readFileSync(files[0], 'utf8'), fs.readFileSync(files[1], 'utf8'));
    if (r.equal) console.log(`EQUAL ${r.states} state lines`);
    else {
      console.log(`DIFFERENT (${r.problems.length} problems)\n  ${r.problems.join('\n  ')}`);
      process.exit(1);
    }
    process.exit(0);
  }
  const r = compare(fs.readFileSync(files[0], 'utf8'), fs.readFileSync(files[1], 'utf8'), { defaults: parseDefaults(args) });
  if (r.equal) console.log(`EQUAL states at ${r.boundaries} boundaries + end (ops: ${r.opsA} vs ${r.opsB})`);
  else {
    console.log(`DIFFERENT (ops: ${r.opsA} vs ${r.opsB})\n  ${r.problems.join('\n  ')}`);
    process.exit(1);
  }
}
