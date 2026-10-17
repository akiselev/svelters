// Probe (runtime research): a minimal push-pull signals core in the shape an own svelters runtime
// could use (Svelte's algorithm: CLEAN / MAYBE_DIRTY / DIRTY colouring, write versions, dynamic
// dependency arrays reused by position, effects queued and flushed in creation order). No batches,
// no async, no boundaries, no dev: it measures the floor of the core loop on QuickJS, it is not a
// candidate implementation. Same adapter surface as svelte-entry.mjs.
const CLEAN = 0, MAYBE_DIRTY = 1, DIRTY = 2;
const DERIVED = 4, EFFECT = 8;

let active = null; // reaction being (re)computed
let newDeps = null; // deps collected past the reused prefix
let skipped = 0; // length of the reused prefix of active.deps
let clock = 0;
let queue = [];
let scheduled = false;

class Node {
  constructor(flags, v, fn) {
    this.f = flags; // kind | colour
    this.v = v;
    this.fn = fn;
    this.deps = null;
    this.reactions = null;
    this.wv = 0;
  }
}

export const name = 'mini';
export const signal = (v) => new Node(CLEAN, v, null);
export const computed = (fn) => new Node(DERIVED | DIRTY, undefined, fn);

export function read(s) {
  if (active !== null) {
    const deps = active.deps;
    if (newDeps === null && deps !== null && deps[skipped] === s) skipped++;
    else if (newDeps === null) newDeps = [s];
    else if (newDeps[newDeps.length - 1] !== s) newDeps.push(s);
  }
  if ((s.f & DERIVED) !== 0 && (s.f & 3) !== CLEAN && isDirty(s)) recompute(s);
  return s.v;
}

function isDirty(r) {
  if ((r.f & 3) === DIRTY) return true;
  const deps = r.deps;
  if (deps !== null) {
    for (let i = 0; i < deps.length; i++) {
      const d = deps[i];
      if ((d.f & DERIVED) !== 0 && (d.f & 3) !== CLEAN && isDirty(d)) recompute(d);
      if (d.wv > r.wv) return true;
    }
  }
  r.f &= ~3; // nothing changed upstream: CLEAN
  return false;
}

function run(r) {
  const pa = active, pn = newDeps, ps = skipped;
  active = r; newDeps = null; skipped = 0;
  try {
    return r.fn();
  } finally {
    const old = r.deps;
    if (newDeps !== null) {
      if (old !== null) unlink(r, old, skipped);
      const deps = old !== null && skipped > 0 ? old.slice(0, skipped).concat(newDeps) : newDeps;
      for (let i = skipped; i < deps.length; i++) {
        const d = deps[i];
        if (d.reactions === null) d.reactions = [r];
        else if (d.reactions[d.reactions.length - 1] !== r) d.reactions.push(r);
      }
      r.deps = deps;
    } else if (old !== null && skipped < old.length) {
      unlink(r, old, skipped);
      old.length = skipped;
    }
    active = pa; newDeps = pn; skipped = ps;
  }
}

function unlink(r, deps, from) {
  for (let i = from; i < deps.length; i++) {
    const rs = deps[i].reactions;
    if (rs === null) continue;
    const j = rs.lastIndexOf(r);
    if (j >= 0) { rs[j] = rs[rs.length - 1]; rs.pop(); }
  }
}

function recompute(d) {
  const v = run(d);
  d.f &= ~3;
  if (!Object.is(v, d.v)) { d.v = v; d.wv = ++clock; } else if (d.wv === 0) d.wv = ++clock;
}

function mark(s, colour) {
  const rs = s.reactions;
  if (rs === null) return;
  for (let i = 0; i < rs.length; i++) {
    const r = rs[i];
    const c = r.f & 3;
    if (c >= colour) continue;
    r.f = (r.f & ~3) | colour;
    if ((r.f & DERIVED) !== 0) { if (c === CLEAN) mark(r, MAYBE_DIRTY); }
    else if (c === CLEAN) schedule(r);
  }
}

function schedule(e) {
  queue.push(e);
  if (!scheduled) { scheduled = true; queueMicrotask(flush); }
}

export function write(s, v) {
  if (Object.is(s.v, v)) return;
  s.v = v;
  s.wv = ++clock;
  mark(s, DIRTY);
}

export function flush() {
  scheduled = false;
  while (queue.length) {
    const q = queue;
    queue = [];
    for (let i = 0; i < q.length; i++) {
      const e = q[i];
      if ((e.f & 3) !== CLEAN && isDirty(e)) { run(e); e.f &= ~3; e.wv = ++clock; }
    }
  }
}

export const root = (fn) => fn();
export function effect(fn) {
  const e = new Node(EFFECT | CLEAN, undefined, fn);
  run(e);
  e.wv = ++clock;
  return e;
}
