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
   * `'cursor-position'` works too. Classes with a native `peek` method of their own (Qt `QIODevice`,
   * `Gio.UnixFDList`) shadow it: use Svelte's `untrack(() => obj.prop)` there.
   */
  peek(prop: any, ...rest: any[]): any;
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

/**
 * Options of an offloaded Rust export (`#[js(worker)]`, `async fn`, ...) whose Rust side reports progress
 * through `WorkCx<P>`: `onProgress` receives the latest value, at most once per host checkpoint. An abort
 * of `signal` rejects the Promise at once with `NativeError { domain: 'svelters', code: 'cancelled' }`.
 */
export interface WorkOptions<P> extends AsyncOptions {
  onProgress?: ((progress: P) => void) | null;
}

/**
 * A Rust stream (an app export with a `Sink<T>` parameter or a `Stream<T>` result): an async iterable.
 * Items arrive at host checkpoints; the producer runs ahead by at most its buffer. Leaving a `for await`
 * early, `close()` or an abort of `options.signal` closes it.
 */
export class NativeStream<T> implements AsyncIterable<T> {
  private constructor();
  next(): Promise<IteratorResult<T, undefined>>;
  return(): Promise<IteratorResult<T, undefined>>;
  [Symbol.asyncIterator](): NativeStream<T>;
  /** The items as arrays, one per host checkpoint that delivered some. */
  batches(): AsyncGenerator<T[], void, undefined>;
  /** Closes the stream: the producer's next `send` fails. */
  close(): void;
}

/**
 * The app object persisted under `key` (it survives hot reloads), or `make()`'s result, persisted now.
 * Objects that are not persisted die with their JS runtime.
 */
export function persist<T extends NativeObject>(key: string, make: () => T): T;
/** Forgets the object persisted under `key`. */
export function unpersist(key: string): void;
/** Registers generated classes so `persist` gives objects back as their class. */
export function registerClass(...classes: Array<{ readonly native: string }>): void;

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
  /** Subscribes the running effect to a native signal of `obj`, as the accessor `prop`. */
  signal?(obj: NativeObject, prop: string, signal: string): void;
  /** Runs `fn` without subscribing. */
  untrack<T>(fn: () => T): T;
}

/** Installs (or with `null` removes) the tracker. Without one, every getter is a plain read. */
export function installTracker(t: Tracker | null): void;
/** A reactive read of an observable property (REACTIVITY.md section 7). */
export function observed(self: NativeObject, name: string): any;
/** A reactive read of a `set_only` property: re-runs on our own writes only. */
export function observedLocal(self: NativeObject, name: string): any;
/** A reactive accessor backed by a change signal and a read function. */
export function observedSignal<T>(self: NativeObject, prop: string, signal: string, read: () => T): T;
/** Re-runs the readers of (object, prop) now. */
export function invalidate(self: NativeObject, name: string): void;
/** `fn()` without reactive subscriptions. */
export function untracked<T>(fn: () => T): T;
