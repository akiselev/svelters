/** Per-toolkit translations of one component stylesheet (produced by the build). */
export interface SheetSet {
  gtk?: string;
  qt?: string;
  qtDark?: string;
  /** The scoped sheet as CSS, for a backend that runs the Rust CSS engine (egui, skelter). */
  css?: string;
  /** skelter `css_precompiled` (release builds): the scoped sheet precompiled, base64 `svelters_style::binary`. */
  bin?: string;
  /** Setter rules per toolkit (setter and child-attr properties, applied with setProp / setChildAttr). */
  rules?: Partial<Record<'gtk' | 'qt', SetterRule[]>>;
}
/** One compound selector (type, classes, id) and the native values of its setter declarations. */
export interface SetterRule {
  sel: { tag: string | null; classes: string[]; id: string | null };
  /** (ids, classes, types) */
  spec: [number, number, number];
  order: number;
  /** [style property, native value], in source order */
  decls: [string, unknown][];
}
/** Registers a component's translated stylesheets under `id`; registering again replaces them. */
export function registerStyles(id: string, sheets: SheetSet): void;
/** Overrides `--sv-*` tokens app-wide; `null` removes an override. Applied once at the end of the job. */
export function setTokens(overrides: Record<string, string | number | null | undefined>): void;
/** The active token set (defaults for the colour scheme plus overrides). */
export function getTokens(): Record<string, string>;
/** Switches the colour scheme: tokens, the Qt palette and `prefers-color-scheme` rules follow. */
export function setColorScheme(scheme: 'light' | 'dark'): void;
export function getColorScheme(): 'light' | 'dark';
