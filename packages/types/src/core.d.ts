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
  /**
   * A snapshot of a property: the getter's value without subscribing the running effect to its changes
   * (getters of observable properties are reactive inside effects). A native spelling such as
   * `'cursor-position'` works too.
   */
  peek<K extends keyof this & string>(prop: K): this[K];
  peek(prop: string): unknown;
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

/** Options of every generated async method: `signal` cancels the native operation (`GCancellable`). */
export interface AsyncOptions {
  /** An `AbortSignal` (structurally: anything with `aborted` and an `abort` event). */
  signal?: { readonly aborted: boolean; addEventListener(type: 'abort', fn: () => void): void } | null;
}

/** An element (a renderer node) where a native widget is expected. */
export interface NativeElement {
  readonly svlHandle: number | null;
}

/** The tracker generated getters subscribe through (installed by `@svelters/native-runtime`). */
export interface Tracker {
  /** Subscribes the running effect to native changes of (handle, prop); no-op outside effects. */
  track(h: number, prop: string): void;
  /** Subscribes the running effect to our own writes of (handle, prop) only (`set_only` props). */
  local(h: number, prop: string): void;
  /** Re-runs the readers of (handle, prop) now. */
  invalidate(h: number, prop: string): void;
  /** Runs `fn` without subscribing. */
  untrack<T>(fn: () => T): T;
}

/** Installs (or with `null` removes) the tracker. Without one, every getter is a plain read. */
export function installTracker(t: Tracker | null): void;
/** A reactive read of an observable property (REACTIVITY.md section 7). */
export function observed(self: NativeObject, name: string): any;
/** A reactive read of a `set_only` property: re-runs on our own writes only. */
export function observedLocal(self: NativeObject, name: string): any;
/** Re-runs the readers of (object, prop) now. */
export function invalidate(self: NativeObject, name: string): void;
/** `fn()` without reactive subscriptions. */
export function untracked<T>(fn: () => T): T;
