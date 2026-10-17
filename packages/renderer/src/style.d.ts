/** Per-toolkit translations of one component stylesheet (produced by the build). */
export interface SheetSet {
  gtk?: string;
  qt?: string;
  qtDark?: string;
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
