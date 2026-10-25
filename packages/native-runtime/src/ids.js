// W-C1 (plans/research/presto/compiler-and-js.md §3.1): stable ids. A module compiled for a
// backend with the `ids` capability names kinds and props by the spec's stable integer ids
// (`$n.element_id($$K_Label)`, `$n.setp(node, $$P_Label_text, v)`) and declares them once with
// `ids(backend, specHash, [[id, tag], [id, tag, prop], ...])`. Before the first use, the ids are
// checked against the host: by its `specHash()` when it has one, else each id against the host's
// own resolvers (`kindId`, `propId`), so a module compiled for another build of the backend fails
// with both hashes named instead of writing the wrong props. Op logs stay by name (the host logs
// names), and until the host takes ids on its fast path (lane RUNTIME) the writes go through
// `element`/`set` by name.
import { element, set } from './index.js';

const host = () => globalThis.__svelters_host;
const kindTags = new Map(); // kind id -> tag
const propNames = new Map(); // prop id -> JS name
let pending = []; // tables not checked against the host yet
const checkedHashes = new Set();

/** Declares a module's ids (checked against the host before the first use). */
export function ids(backend, hash, table) {
  for (const [id, tag, prop] of table) {
    if (prop === undefined) kindTags.set(id, tag);
    else propNames.set(id, prop);
  }
  pending.push({ backend, hash, table });
}

function check() {
  const hs = host();
  for (const { backend, hash, table } of pending) {
    if (checkedHashes.has(hash)) continue;
    const own = typeof hs.specHash === 'function' ? hs.specHash() : undefined;
    if (own !== undefined && own !== null) {
      if (own !== hash) throw new Error(`svelters: this module was compiled for ${backend} spec ${hash}, but the host's spec is ${own}: recompile it`);
      checkedHashes.add(hash);
      continue;
    }
    for (const [id, tag, prop] of table) {
      const kind = hs.kindId(tag);
      const have = prop === undefined ? kind : hs.propId(kind, prop);
      if (have !== id) {
        const what = prop === undefined ? `<${tag}>` : `\`${prop}\` of <${tag}>`;
        throw new Error(`svelters: this module was compiled for ${backend} spec ${hash}, where ${what} is id ${id}; the host's is ${have}: recompile it`);
      }
    }
  }
  pending = [];
}

/** A native element by kind id. */
export function element_id(id) {
  if (pending.length) check();
  const tag = kindTags.get(id);
  if (tag === undefined) throw new Error(`svelters: unknown kind id ${id}`);
  return element(tag);
}

/** `set` by prop id. */
export function setp(node, id, value) {
  if (pending.length) check();
  const name = propNames.get(id);
  if (name === undefined) throw new Error(`svelters: unknown prop id ${id}`);
  return set(node, name, value);
}
