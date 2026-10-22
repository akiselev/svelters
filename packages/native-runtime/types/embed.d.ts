/// <reference path="./embed-module.d.ts" />
// `svelters:embed` and `@svelters/native-runtime/embed` (EM1, plans/research/ARCH-EMBED.md §4.3): what a
// component mounted by a native embedder (svelters-egui's `Embed`) sees of its root.
//
// Use it from an app's tsconfig next to the element types:
//   "types": ["@svelters/native-runtime/types", "@svelters/native-runtime/embed"]
// which also declares `svelters:embed` (`embed-module.d.ts`).

export interface RootHandle {
  /** The host's root id. */
  readonly id: number;
  /** The host's stable name for the root (`MountSpec::key`). */
  readonly key: string;
  /** False while the host hides the root's slot: its animation frames wait (SV19). */
  readonly visible: boolean;
  /** The slot's last laid-out size, in points, and the pixels per point of its viewport. */
  readonly width: number;
  readonly height: number;
  readonly scale: number;
  /** The viewport (OS window) the slot is drawn in. */
  readonly viewport: string;
  readonly focused: boolean;
  /** Read by the host for the pane's tab (`EmbedEvent::InfoChanged`). */
  title: string;
  dirty: boolean;
  /** Sends `EmbedEvent::Emitted { name, value }` to the host; `value` must be JSON-serializable. */
  emit(name: string, value?: unknown): void;
  /** `fn` runs when the host asks the root to close; `false` keeps it open. Returns the remover. */
  onCloseRequest(fn: () => boolean | void): () => void;
}

/** The calling component's root (call it during component initialisation). */
export function root(): RootHandle;

/** The context key `root()` reads. */
export const ROOT_KEY: unique symbol;

