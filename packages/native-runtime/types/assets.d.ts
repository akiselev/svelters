// AS1 (plans/research/ASSETS.md §1.3, AS-O1 (b)): asset imports and `svelters:assets`.
// Reference it from the app's tsconfig (`"types": ["@svelters/native-runtime/assets"]`) instead of
// `vite/client`, whose asset declarations type the same patterns as `string`.

/// <reference path="./asset-imports.d.ts" />

/** An imported asset: a handle on a stable `asset://<root>/<path>` ref. It crosses into native
 * props, Rust exports and `JSON.stringify` as its ref. */
export declare class Asset {
  private constructor();
  /** `asset://<root>/<path>`. */
  readonly url: string;
  readonly root: string;
  readonly path: string;
  readonly mime: string;
  /** Bytes. */
  readonly size: number;
  info(): { mime: string; len: number; hash: string; version: number };
  text(): string;
  bytes(): Uint8Array;
  /** Dev: called after each change of the file. Returns the unsubscribe function. */
  onChange(f: (a: Asset) => void): () => void;
  toString(): string;
  toJSON(): string;
}
