// The value of `bind:this` on a native element: the renderer's node, with native access.

export interface NativeElement {
  /** The element tag, e.g. `entry`. */
  readonly tag: string;
  /** The native handle while the widget exists, else null. */
  readonly handle: number | null;
  /** Why the native widget is gone ('destroyed' or a native death reason), else null. */
  readonly dead: string | null;
  /** Call a native method, e.g. `window.call('present')`. Throws StaleHandleError when dead. */
  call(method: string, ...args: unknown[]): unknown;
  /** Read a native property, e.g. `entry.getProp('text')`. Throws StaleHandleError when dead. */
  getProp(name: string): unknown;
}

export declare class StaleHandleError extends Error {
  readonly name: 'StaleHandleError';
  /** 'destroyed' once the renderer destroyed the widget, or the native death reason. */
  readonly reason: string;
}
