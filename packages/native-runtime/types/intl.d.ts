// `svelters:intl` and `@svelters/native-runtime/intl` (IN1): the host's locale as a store.
// Ambient `svelters:intl` lives in `intl-module.d.ts`.
/// <reference path="./intl-module.d.ts" />

/** The host's current locale (a BCP 47 tag such as `de-DE`). */
export function getLocale(): string;

/** The host's locale as a readable Svelte store; `$locale` re-renders on `Host::set_locale`. */
export const locale: { subscribe(run: (tag: string) => void): () => void };
