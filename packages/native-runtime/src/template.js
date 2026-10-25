// W-C2 (plans/research/presto/compiler-and-js.md §3.2): native templates, the runtime side.
//
// A module compiled for a backend with the `templates` capability declares each element subtree
// whose static part the compiler knows once, at module scope (`template(hash, nodes, listeners)`),
// and builds an instance with one `instantiate(t, container, thunks, file)`; the nodes later code
// uses come from `at(instance, k)`, and the template's listeners are registered after the
// fragment's template effect with one `template_listen(instance, handlers)`.
//
// The table (crates/svelters-compiler/src/codegen/component/template.rs): `nodes` in pre-order,
// each `[kind, slot, ops, children, loc]`; an op is `[name, ...operands]` (`set`, `class`, `style`,
// `id`, `cattr`, `token`, `track`, and `setv`, `idv`, `cattrv` whose last operand indexes `thunks`);
// a child is a node number, or a hole `[thunk, slot]` that builds a block into the slot's
// container; `loc` is the node's source offset (dev builds, DZ0) or null. `listeners` are
// `[node, signal, flags]`, in the order of `template_listen`'s handlers.
//
// Instances are built by expanding the table into the ops the compiler writes for elements without
// templates, in the same order (create, source location, ops, children, insert), so every host
// works and op logs are unchanged. A host-side `instantiate` (one native call per instance) needs
// the runtime to adopt the nodes it creates (lane RUNTIME, plans/milestones/PRESTO-compiler.md).
import { append, attr, child_attr, element, on, set, slot, source_loc, token, track } from './index.js';

const templates = new Map(); // content hash -> template

/** The template of a module-scope table, shared by every module that declares the same content. */
export function template(hash, nodes, listeners) {
  let t = templates.get(hash);
  if (!t) templates.set(hash, (t = { hash, nodes, listeners }));
  return t;
}

/** Builds an instance of `t` into `container`: its nodes, by number (`at`). */
export function instantiate(t, container, thunks = [], file = undefined) {
  const nodes = new Array(t.nodes.length);
  nodes.template = t;
  build(t, 0, container, null, thunks, file, nodes);
  return nodes;
}

/** Node `k` of an instance. */
export function at(instance, k) {
  return instance[k];
}

/** The listeners of an instance's template, `handlers` in its slot order. */
export function template_listen(instance, handlers) {
  const ls = instance.template.listeners;
  for (let i = 0; i < ls.length; i++) on(instance[ls[i][0]], ls[i][1], handlers[i], ls[i][2]);
}

// a static value as a literal would be: a fresh array or object per write
const fresh = (v) => (v !== null && typeof v === 'object' ? JSON.parse(JSON.stringify(v)) : v);

function build(t, k, container, parent, thunks, file, nodes) {
  const [kind, slotName, ops, children, loc] = t.nodes[k];
  const n = (nodes[k] = element(kind));
  if (loc !== null && file !== undefined) source_loc(n, file, loc);
  for (const op of ops) apply(n, op, thunks);
  for (const c of children) {
    if (typeof c === 'number') build(t, c, null, n, thunks, file, nodes);
    else thunks[c[0]](slot(n, c[1]));
  }
  append(container ?? slot(parent, slotName), n);
}

function apply(n, op, thunks) {
  switch (op[0]) {
    case 'set':
      return void set(n, op[1], fresh(op[2]));
    case 'setv':
      return void set(n, op[1], thunks[op[2]]());
    case 'class':
      return void attr(n, 'class', op[1]);
    case 'style':
      return void attr(n, 'style', op[1]);
    case 'id':
      return void attr(n, 'id', op[1]);
    case 'idv':
      return void attr(n, 'id', thunks[op[1]]());
    case 'cattr':
      return void child_attr(n, op[1], fresh(op[2]));
    case 'cattrv':
      return void child_attr(n, op[1], thunks[op[2]]());
    case 'token':
      return void token(n, op[1], op[2], true);
    case 'track':
      return void track(n, op[1], op[2], op[3]);
    default:
      throw new Error(`svelters: unknown template op ${JSON.stringify(op[0])}`);
  }
}
