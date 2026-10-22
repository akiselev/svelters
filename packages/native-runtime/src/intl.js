// `svelters:intl` (IN1): the host's locale. svelters ships no translation catalog: the host
// (cadatomic) owns the locale and its strings and switches language with `Host::set_locale`.
//
//   <script>
//     import { locale } from 'svelters:intl';
//   </script>
//   <label>{$locale}: {(1234.5).toLocaleString($locale)}</label>
//
// `locale` is a Svelte store (the store contract: `subscribe(run) -> unsubscribe`), so `$locale`
// re-renders when the host changes language. `Intl` and `toLocaleString` with no locale argument
// use the new locale too, but a component re-renders only for what it reads: read `$locale` (or
// pass it as the locale argument) wherever text depends on the language.
//
// Outside the svelters host (Node tests, the browser) the store holds the environment's locale
// and never changes.

const api = () => globalThis.__svelters_intl;

const fallback = () => {
  try {
    return new Intl.NumberFormat().resolvedOptions().locale;
  } catch (_) {
    return 'en-US';
  }
};

/** The host's current locale (a BCP 47 tag). */
export function getLocale() {
  const a = api();
  return a ? a.locale : fallback();
}

/** The host's locale as a readable store. */
export const locale = {
  subscribe(run) {
    const a = api();
    if (a) return a.subscribe(run);
    run(fallback());
    return () => {};
  },
};
