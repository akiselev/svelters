// Cached host member resolution for the renderer's hot ops.
//
// A host that offers `kindId`/`propId`/`methodId` (the QuickJS host does) takes ids on `create`,
// `setProp` and `call`, which skips a name lookup and a string conversion per op (measured:
// 1.43 -> 0.77 us per setProp). Everything is resolved once per (kind, name) and cached per host.
// A host without those functions (or a name it cannot resolve) just gets the name, so the
// JS-facing API is unchanged and the host still produces the error for unknown names.

const caches = new WeakMap(); // host -> { kinds, props, methods, types }

function cacheOf(host) {
  let c = caches.get(host);
  if (!c) {
    c = { kinds: new Map(), props: new Map(), methods: new Map(), types: new Map() };
    caches.set(host, c);
  }
  return c;
}

function resolve(map, key, fn) {
  if (map.has(key)) return map.get(key);
  let id = null;
  try {
    const v = fn();
    if (v !== undefined) id = v;
  } catch {
    // Unknown here: the op itself will report it by name.
  }
  map.set(key, id);
  return id;
}

/** The kind id for `tag`, or `tag` itself when the host cannot resolve ids. */
export function kindRef(host, tag) {
  if (typeof host.kindId !== 'function') return tag;
  return resolve(cacheOf(host).kinds, tag, () => host.kindId(tag)) ?? tag;
}

function memberRef(host, tag, name, fn, store) {
  if (typeof host.kindId !== 'function' || typeof host[fn] !== 'function') return name;
  const c = cacheOf(host);
  const kid = resolve(c.kinds, tag, () => host.kindId(tag));
  if (kid === null) return name;
  return resolve(c[store], `${kid}\u0000${name}`, () => host[fn](kid, name)) ?? name;
}

/** The prop id for `name` on kind `tag`, or `name` when unresolved. */
export const propRef = (host, tag, name) => memberRef(host, tag, name, 'propId', 'props');

/** The method id for `name` on kind `tag`, or `name` when unresolved. */
export const methodRef = (host, tag, name) => memberRef(host, tag, name, 'methodId', 'methods');

/**
 * The coarse value type of a prop (`integer`, `number`, `boolean`, `string`, `enum`, `flags`,
 * `record`, ...) when the host's backend publishes descriptors, else null.
 */
export function propType(host, tag, name) {
  if (typeof host.propType !== 'function') return null;
  const ref = propRef(host, tag, name);
  if (typeof ref !== 'number') return null;
  const kid = kindRef(host, tag);
  return resolve(cacheOf(host).types, `${kid}\u0000${ref}`, () => host.propType(kid, ref)) ?? null;
}
