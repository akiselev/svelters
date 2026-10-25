// W-C1 (plans/research/presto/compiler-and-js.md §3.1): stable ids. A module compiled for a
// backend with the `ids` capability names kinds and props by the spec's stable integer ids
// (`$n.element_id($$K_Label)`, `$n.setp(node, $$P_Label_text, v)`; `setp` is index.js's scalar
// fast path, which takes a name or an id) and declares them once with
// `ids(backend, specHash, [[id, tag], [id, tag, prop], ...])`. The ids are checked against the
// host when they are declared (or, without a host yet, before the first `element_id`): by the
// spec hash the backend registered (`host.specHash(backend)`), else each id against the host's
// own resolvers (`kindId`, `propId`), so a module compiled for another build of the backend fails
// with both hashes named instead of writing the wrong props. Op logs stay by name (the host logs
// names).
import { element } from './index.js';

const host = () => globalThis.__svelters_host;
const kindTags = new Map(); // kind id -> tag
let pending = []; // tables not checked against the host yet
const checkedHashes = new Set();

/** Declares a module's ids (checked against the host now, or before the first use). */
export function ids(backend, hash, table) {
  for (const [id, tag, prop] of table) if (prop === undefined) kindTags.set(id, tag);
  pending.push({ backend, hash, table });
  if (host()) check();
}

function check() {
  const hs = host();
  const tables = pending;
  pending = [];
  for (const { backend, hash, table } of tables) {
    if (checkedHashes.has(hash)) continue;
    const own = typeof hs.specHash === 'function' ? hs.specHash(backend) : undefined;
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
}

/** A native element by kind id. */
export function element_id(id) {
  if (pending.length) check();
  const tag = kindTags.get(id);
  if (tag === undefined) throw new Error(`svelters: unknown kind id ${id}`);
  return element(tag);
}
