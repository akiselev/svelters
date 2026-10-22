// svelters:assets (AS1, plans/research/ASSETS.md §1, AS-O1 (b)): asset handles and reads.
//
// `import icon from './icon.svg'` gives an `Asset` handle. It wraps a stable ref
// (`asset://app/src/icon.svg`) that names a logical path, not content, so it does not change when
// the file does. Wherever a handle reaches native code (a prop such as `image.source`, an export
// argument, `Json<T>`), the host takes its ref from the hidden `__svelters_asset` property;
// `String(icon)`, template literals and `JSON.stringify` give the ref too.

const KEY = '__svelters_asset';
const host = () => {
  const h = globalThis.__svelters_host;
  if (!h || typeof h.assets !== 'function') throw new Error('svelters:assets: the host has no asset registry');
  return h;
};
const handles = new Map();
const listeners = new Map();

const refOf = (a) => {
  if (a instanceof Asset) return a[KEY];
  if (typeof a === 'string' && a.startsWith('asset://')) return a;
  throw new TypeError(`expected an Asset or an asset:// ref, got ${a === null ? 'null' : typeof a}`);
};

export class Asset {
  constructor(ref) {
    if (typeof ref !== 'string' || !ref.startsWith('asset://')) throw new TypeError(`not an asset ref: ${String(ref)}`);
    Object.defineProperty(this, KEY, { value: ref });
    Object.freeze(this);
  }
  /** The ref, `asset://<root>/<path>`: an image source, a drag payload, a manifest field. */
  get url() { return this[KEY]; }
  get root() { return this[KEY].slice(8, this[KEY].indexOf('/', 8)); }
  get path() { return this[KEY].slice(this[KEY].indexOf('/', 8) + 1); }
  /** `{ mime, len, hash, version }`, read now (version counts dev changes). */
  info() { return JSON.parse(host().assets('info', this[KEY])); }
  get mime() { return this.info().mime; }
  get size() { return this.info().len; }
  /** The content as UTF-8 text. */
  text() { return host().assets('text', this[KEY]); }
  /** The content, copied once from Rust. */
  bytes() { return new Uint8Array(host().assetBytes(this[KEY])); }
  /** Dev: `f(asset)` after each change of the file. Returns the unsubscribe function. */
  onChange(f) {
    const ref = this[KEY];
    let set = listeners.get(ref);
    if (!set) listeners.set(ref, (set = new Set()));
    set.add(f);
    return () => set.delete(f);
  }
  toString() { return this[KEY]; }
  toJSON() { return this[KEY]; }
  [Symbol.toPrimitive]() { return this[KEY]; }
  get [Symbol.toStringTag]() { return 'Asset'; }
}

/** The handle of a ref (one handle per ref), or of a path under the app root (`icons/gear.svg`). */
export function asset(p) {
  const ref = typeof p === 'string' && !p.startsWith('asset://') ? `asset://app/${p.replace(/^\.?\//, '')}` : refOf(p);
  let h = handles.get(ref);
  if (!h) handles.set(ref, (h = new Asset(ref)));
  return h;
}

export const text = (a) => host().assets('text', refOf(a));
export const bytes = (a) => new Uint8Array(host().assetBytes(refOf(a)));
export const isAsset = (a) => a instanceof Asset;

// Rust exports that return an `AssetRef` wrap the ref with this (the generated `svelters:app/*` code).
Object.defineProperty(globalThis, '__svelters_asset', { configurable: true, value: asset });

// Called by the host after asset files changed (dev reload), with their refs.
Object.defineProperty(globalThis, '__svelters_assets_changed', {
  configurable: true,
  value: (refs) => {
    for (const r of refs) {
      const set = listeners.get(r);
      if (set) for (const f of [...set]) f(asset(r));
    }
  },
});
