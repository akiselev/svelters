#!/usr/bin/env node
// Compares two op logs (plans/architecture/backend-abi.md "Op-log format") by the native state
// they produce, not by their text: each log is replayed into a tree, and the states are compared
// before every `event` / `sync` / `deferred` / `gone` line and at the end (the values JS returned for `sync`
// lines, the `result` lines, are part of the state). A state is: live handles with kind, props,
// children per slot (in order), listened signals and the methods called on them.
//
// Usage: node tools/oplog-compare.mjs <a.oplog> <b.oplog> [--default prop=<json>]...
//   --default label=""   a prop set to this value counts as never set (e.g. a label written "")
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
    case 'set': {
      const [, n, prop, json] = sp(line, 4);
      return { op, h: h(n), prop, value: JSON.parse(json) };
    }
    case 'call': {
      const [, n, method, json] = sp(line, 4);
      return { op, h: h(n), method, args: JSON.parse(json) };
    }
    case 'listen': {
      const [, n, signal, on] = sp(line, 4);
      return { op, h: h(n), signal, on: on === 'on' };
    }
    case 'event': {
      const [, n, signal, json] = sp(line, 4);
      return { op, h: h(n), signal, args: JSON.parse(json) };
    }
    case 'gone': {
      const [, n, reason] = sp(line, 3);
      return { op, h: h(n), reason };
    }
    case 'deferred':
    case 'sync': {
      const [, n, signal, json] = sp(line, 4);
      return { op, h: h(n), signal, args: JSON.parse(json) };
    }
    case 'result': {
      const [, n, json] = sp(line, 3);
      return { op, h: h(n), value: JSON.parse(json) };
    }
    default:
      throw new Error(`unknown op: ${line}`);
  }
}

/** Replays `text`; returns { boundaries: [{ line, state }], final, ops } (state = canonical JSON). */
export function replay(text, { defaults = {} } = {}) {
  const nodes = new Map(); // h -> { kind, props, parent, slots: Map, listening: Set, calls: [] }
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
          .filter(([k, v]) => !(k in defaults && Object.is(defaults[k], v)))
          .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)),
      );
      const slots = Object.fromEntries([...n.slots].sort(([a], [b]) => (a < b ? -1 : 1)));
      return { h, kind: n.kind, props, slots, listening: [...n.listening].sort(), calls: n.calls };
    });
    if (results.length) out.push({ h: 'results', results });
    return JSON.stringify(out);
  };

  const boundaries = [];
  const results = []; // sync results, in order
  const lines = text.split('\n').filter(Boolean);
  for (const line of lines) {
    const o = parseLine(line);
    switch (o.op) {
      case 'create':
        if (nodes.has(o.h)) throw new Error(`handle reused: ${line}`);
        nodes.set(o.h, { kind: o.kind, props: new Map(), parent: null, slots: new Map(), listening: new Set(), calls: [] });
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
        get(o.h, line).props.set(o.prop, o.value);
        break;
      case 'call':
        get(o.h, line).calls.push(`${o.method} ${JSON.stringify(o.args)}`);
        break;
      case 'listen':
        if (o.on) get(o.h, line).listening.add(o.signal);
        else get(o.h, line).listening.delete(o.signal);
        break;
      case 'result':
        // the value JS returned to the toolkit for the preceding `sync`: part of the next state
        results.push(`${o.h} ${JSON.stringify(o.value)}`);
        break;
      case 'event':
      case 'sync':
      case 'deferred':
      case 'gone':
        boundaries.push({ line, state: snapshot() });
        if (o.op === 'gone') kill(o.h);
        break;
    }
  }
  return { boundaries, final: snapshot(), ops: lines.filter((l) => !/^(event|sync|deferred|result|gone) /.test(l)).length };
}

function firstDiff(a, b) {
  const x = JSON.parse(a);
  const y = JSON.parse(b);
  const bx = new Map(x.map((n) => [n.h, n]));
  const by = new Map(y.map((n) => [n.h, n]));
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
    console.error('usage: oplog-compare.mjs <a.oplog> <b.oplog> [--default prop=<json>]...');
    process.exit(2);
  }
  const r = compare(fs.readFileSync(files[0], 'utf8'), fs.readFileSync(files[1], 'utf8'), { defaults: parseDefaults(args) });
  if (r.equal) console.log(`EQUAL states at ${r.boundaries} boundaries + end (ops: ${r.opsA} vs ${r.opsB})`);
  else {
    console.log(`DIFFERENT (ops: ${r.opsA} vs ${r.opsB})\n  ${r.problems.join('\n  ')}`);
    process.exit(1);
  }
}
