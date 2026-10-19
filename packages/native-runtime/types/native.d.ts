// The value of `bind:this` on a native element: the runtime's node, with native access.

export interface NativeElement {
  /** The element tag, e.g. `entry`. */
  readonly tag: string;
  /** The native handle while the widget exists, else null. */
  readonly handle: number | null;
  /** Why the native widget is gone ('destroyed' or a native death reason), else null. */
  readonly dead: string | null;
  /** True for a native object the app did not create (a parent, a root, an internal child). Such nodes are never part of the tree: inserting, removing or listening on them throws. */
  readonly borrowed?: boolean;
  /**
   * Call a native method, e.g. `window.call('present')`. Throws StaleHandleError when dead.
   * An object result is a node: the bound node for an object the app created, else one stable
   * borrowed node per native object. Nodes are accepted as arguments.
   */
  call(method: string, ...args: unknown[]): unknown;
  /** Read a native property, e.g. `entry.getProp('text')`. Throws StaleHandleError when dead. Object results are nodes, as for `call`. */
  getProp(name: string): unknown;
}

export declare class StaleHandleError extends Error {
  readonly name: 'StaleHandleError';
  /** 'destroyed' once the runtime destroyed the widget, or the native death reason. */
  readonly reason: string;
}
