// Types of the object core under the generated `svelters:<toolkit>/<ns>` modules.

/** Thrown when an object or value is used after `dispose()`. */
export class DisposedError extends Error {
  readonly reason: 'disposed';
  constructor(what: string);
}

/** A native type: a generated class or its native name (`'Gio.File'`). */
export type NativeType = string | { readonly native: string };

/**
 * One JS object per native object. The handle is SHARED: a strong reference held until the wrapper is
 * garbage collected or `dispose()` is called.
 */
export class NativeObject {
  /** The handle the host converts an object argument to; null once disposed. */
  readonly svlHandle: number | null;
  readonly disposed: boolean;
  /** The native type name of the class this wrapper was made as. */
  readonly nativeType: string;
  constructor(...args: any[]);
  /** Listens to a native signal (`'items-changed'` or `itemsChanged`). Returns the disposer. */
  on(signal: string, fn: (...args: any[]) => void): () => void;
  /** Releases our reference now. The native object lives on while anything else holds it. */
  dispose(): void;
  [Symbol.dispose](): void;
  /** Rewraps the object as this class (an unchecked cast). */
  static from<T extends NativeObject>(this: new (...args: any[]) => T, obj: NativeObject | null): T | null;
}

/** An immutable native value (a boxed struct) behind a VALUE handle: no identity, `equals`, `toString`. */
export class NativeValue {
  readonly disposed: boolean;
  readonly nativeType: string;
  equals(other: NativeValue | null): boolean;
  dispose(): void;
  [Symbol.dispose](): void;
  static from<T extends NativeValue>(this: new (...args: any[]) => T, value: NativeValue | null): T | null;
}
