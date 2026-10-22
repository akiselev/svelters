# IN1: `Intl` for QuickJS over ICU4X, and locale-aware numeric widgets

QT-SURVEY §2.5 (QA15), milestone IN1. QuickJS-NG has no `Intl`, so `toLocaleString`, plural rules
and collation did not work in Svelte code, and egui's numeric widgets ignored the locale.
Coordinator decision (2026-10-22), with cadatomic's constraint: **the host owns the locale and the
translated strings** (cadatomic's catalog, `catalog.t`). svelters ships no catalog. It provides
`Intl` under the host's locale, locale-aware numeric widgets, and a host API to switch the locale
at runtime.

Branch `ci/in1`.

## 1. API

### Rust: `crates/svelters-intl` (new)

ICU4X 2.4 (`icu_*` crates pinned `=2.4.0`, `icu_experimental =0.7.0`; the unstable field-bag,
compact, percent and currency APIs are why the pins are exact), with no compiled data. Data loads
from one postcard blob (§2).

```rust
pub struct Locale;                         // parse(tag), parse_lenient("de_DE.UTF-8"), tag(), language(), is_supported()
pub fn canonicalize(tag) -> Result<String, IntlError>;
pub fn resolve(requested: &[&str]) -> Result<Locale, IntlError>;   // ECMA-402 lookup against the data's locales
pub fn current() -> Locale;                // the thread's locale -> process default -> system_locale()
pub fn set_current(tag) -> Result<Locale, IntlError>;            // Host::set_locale calls this
pub fn current_generation() -> u64;        // bumps on set_current (for caches of formatted text)
pub fn set_process_default(tag); pub fn system_locale() -> Locale;  // LC_ALL, LC_NUMERIC, LANG; else en-US

pub fn format_number(&Locale, f64, &NumberOptions) -> Result<String, IntlError>;
pub fn format_number_to_parts(..) -> Result<Vec<Part>, IntlError>;      // Part { kind, value }
pub fn parse_number(&Locale, &str) -> Option<f64>;                       // "1,5" -> 1.5 in de-DE
pub fn plural_category(&Locale, f64, &PluralOptions) -> Result<&'static str, IntlError>;
pub fn plural_categories(&Locale, ordinal: bool) -> Result<Vec<&'static str>, IntlError>;
pub fn compare(&Locale, &str, &str, &CollatorOptions) -> Result<Ordering, IntlError>;
pub fn format_date(&Locale, epoch_ms: f64, Zone, &DateOptions) -> Result<String, IntlError>;  // Zone::Utc | Zone::Offset(secs)
pub fn format_date_to_parts(..) -> Result<Vec<Part>, IntlError>;
pub fn use_data(blob: &'static [u8], locales: &[&str]) -> Result<(), IntlError>;   // an embedder's own blob
pub fn data_locales() -> &'static [String];
```

The option structs (`NumberOptions`, `PluralOptions`, `CollatorOptions`, `DateOptions`) are
`#[non_exhaustive]` with public fields and ECMA-402 defaults. Formatters are cached per thread
(bounded maps keyed by locale and options).

`parse_number` accepts the locale's digits, decimal and group separators (any space where the
locale groups with one), its minus sign, `-`/`+`/`−`, and an exponent. When the locale's decimal
separator is not `.` and the text has none, a single `.` that is not followed by exactly three
digits is read as the decimal point (`1.5` is 1.5 in de-DE, `1.500` is 1500).

### Host: `svelters-host`, feature `intl` (on by default)

- `HostConfig::locale: Option<String>`: the starting locale (BCP 47 or POSIX), set as the thread's
  current locale when the host is created. `None` keeps the thread's locale (the environment's).
- `Host::set_locale(&self, tag) -> Result<String, JsError>`: switches the locale at runtime. It sets
  the thread's current locale (read by `Intl` defaults, `toLocaleString` and the egui widgets) and
  tells JS: `__svelters_intl` subscribers run and the cached default formatters are dropped. It is
  delivered now (one entry into JS, then a job drain), or by the next checkpoint if called while JS
  runs (from a Rust export). Returns the canonical tag.
- `Host::locale() -> String`. `svelters_host::intl` re-exports svelters-intl.
- `JsEngine::locale_changed(tag)`, with a no-op default.
- JS: `crates/svelters-host/src/intl.js` (the shim) over the natives in
  `crates/svelters-host/src/quickjs/intl.rs`, installed after the prelude:
  - `Intl.NumberFormat`: decimal, percent and currency styles; `minimumIntegerDigits`,
    `minimum/maximumFractionDigits`, `minimum/maximumSignificantDigits`, `useGrouping`
    (`true`/`false`/`auto`/`always`/`min2`), `signDisplay`, `notation: "compact"` with
    `compactDisplay`; `format` (a bound getter), `formatToParts`, `resolvedOptions`.
  - `Intl.PluralRules`: cardinal and ordinal, fraction digits; `select`, `resolvedOptions`
    (with `pluralCategories`).
  - `Intl.DateTimeFormat`: `dateStyle`/`timeStyle`, and `weekday`, `era`, `year`, `month`, `day`,
    `hour`, `minute`, `second`, `fractionalSecondDigits`, `timeZoneName`, `hour12`/`hourCycle`;
    `format`, `formatToParts`, `resolvedOptions`. Time zones are `UTC` (and its aliases), the host's
    local zone (the default, or its IANA name), and fixed offsets (`+05:30`).
  - `Intl.Collator`: `sensitivity`, `numeric` (also `-u-kn`), `caseFirst`, `ignorePunctuation`;
    `compare` (a bound getter).
  - `Intl.getCanonicalLocales`, `supportedLocalesOf` on every constructor.
  - `Number.prototype.toLocaleString` (and `BigInt`'s), `Date.prototype.toLocaleString`,
    `toLocaleDateString` and `toLocaleTimeString` (ECMA's ToDateTimeOptions defaults), and
    `String.prototype.localeCompare`, all routed to the classes above.
  - Options are validated as ECMA-402 does where that is cheap (`RangeError` for out-of-range
    values and malformed tags, `TypeError` for a missing currency, mixing styles with components,
    and calling `PluralRules` without `new`). The default locale is the host's.
- `svelters:intl` (`packages/native-runtime/src/intl.js`, also `@svelters/native-runtime/intl`,
  through the alias table that Vite and the Rust source loader share): `locale`, a Svelte store
  (`$locale` re-renders on `Host::set_locale`), and `getLocale()`. Outside the host it holds the
  environment's locale.

### egui: `svelters-egui`, feature `intl` (on by default)

`crates/svelters-egui/src/numeric.rs` (IN1's file): `format(value, decimals)` (egui's choice of
decimals, written in the current locale), `parse(text)` (the locale's notation, then egui's), and
`slider()`, `drag_value()` and `range_slider()`, which attach them as egui's
`custom_formatter`/`custom_parser`. The widget files change by one call each: `leaf.rs` (`slider`)
and `elements/inputs.rs` (`drag-value`, and `range-slider`'s value boxes, captioned or not).

## 2. Locale subset and data

- Locales: **en de fr es it ja zh ru pl** (`crates/svelters-intl/data/locales.txt`). Other regions
  of these languages fall back to them (de-AT, en-GB, fr-CA). A requested locale outside the subset
  resolves to the host's locale, or `en` if that is outside too, as ECMA-402 negotiation does.
  cadatomic named no further locales; adding one means regenerating the blob.
- Data: `crates/svelters-intl/data/icu4x.postcard`, **1.76 MB**, committed. It is made by
  `tools/intl/gen-data.sh`, which runs `icu4x-datagen` 2.4.0 (built with `--features unstable`) from
  pinned sources: CLDR 49.0.0, icuexport release-79.1rc and tzdb 2026e, the versions datagen 2.4.0
  calls `latest`. The marker list is what the constructors load. It was found once with
  `--markers-for-bin` on the svelters-intl test binary, then cut to the Gregorian calendar, and is
  recorded in the script. `LOCALES="…" tools/intl/gen-data.sh` makes another subset;
  `OUT=… tools/intl/gen-data.sh` makes an embedder blob for `svelters_intl::use_data`, so cadatomic
  can ship its own locale list without a svelters change.
- Why a blob and not baked data or `compiled_data`: compiled data carries every CLDR locale, and
  baked Rust sources for the subset would be several large files that are slow to compile. The blob
  is one small binary file. It is deserialized lazily, once per process, and the formatters built
  from it are cached per thread.

## 3. Feature defaults and size

`intl` is **on by default** in svelters-host and svelters-egui. The reasons: stock Svelte code and
libraries call `toLocaleString`/`localeCompare`, a missing `Intl` throws, and cadatomic needs the
locale-aware widgets. Without the feature there is no `Intl` global (as before IN1), no
`Host::set_locale`, and the widgets use egui's own `1234.5` notation.

Size delta: **not measured yet** (handoff, §8). The data blob alone is 1.76 MB of the binary; the probe is `crates/svelters-intl/examples/size.rs` (`cargo build --release -p svelters-intl --example size` with and without `--features probe`).

Build cost: about 25 more crates (icu_* and helpers) in every crate that depends on svelters-host
(svelters-compiler does too).

## 4. Deviations from ECMA-402

- **Time zones**: only UTC, the host's local zone (formatted by its offset at the instant, so its
  name shows as `GMT+2`, not `CEST`), and fixed offsets. Any other IANA name is a `RangeError`.
  `resolvedOptions().timeZone` is the host zone's IANA name from `TZ` or `/etc/localtime` (Unix), or
  an offset such as `+02:00` when it is unknown (Windows).
- **Calendar**: Gregorian only (`-u-ca-*` and `calendar` are ignored). The data has no other
  calendars.
- **Numbering systems**: each locale's default (Latin digits for the subset). `numberingSystem`
  and `-u-nu` are read and ignored.
- **Numeric dates follow ICU4X field sets, not V8's skeleton matching**: `toLocaleDateString('de-DE')`
  is `05.03.2024`, where V8 gives `5.3.2024`. Styles (`dateStyle: "long"` and so on) match V8.
  When `dateStyle` and `timeStyle` differ, both use `dateStyle`'s length.
- **NumberFormat**: `style: "unit"` throws `RangeError`. `notation: "scientific"`/`"engineering"`
  format as standard. `roundingMode`, `roundingIncrement`, `roundingPriority` and
  `trailingZeroDisplay` are not read; rounding is always half-expand. `currencySign: "accounting"`
  is accepted but formats as standard. With currency style, the formatter rounds to the currency's
  own digits, so `maximumFractionDigits` above them has no effect.
- **Not provided**: `Intl.Locale`, `Intl.RelativeTimeFormat`, `Intl.ListFormat`,
  `Intl.DisplayNames`, `Intl.Segmenter`, `Intl.DurationFormat`, `formatRange`,
  `PluralRules.selectRange`, `Intl.supportedValuesOf`.
- **Locale canonicalization** fixes case and syntax but applies no CLDR aliases (`iw` stays `iw`).
  Negotiation is lookup by language against the subset, for `"best fit"` too.
- **The default locale** is per thread (the UI thread's, which the host and egui share). A worker
  thread sees `set_process_default` or the environment.
- **Strings with lone surrogates** are compared after a lossy conversion to UTF-8.

## 5. Tests

- `crates/svelters-intl/src/tests.rs`: decimal, percent, currency and compact output in several
  locales; parts; parsing (de, en and fr, plus a round trip in all 9 locales); plurals in en, pl, ru
  and ja, ordinal and with fraction digits; collation (German umlauts, sensitivities, numeric,
  Polish ł); dates (styles in en, de, fr and ja, components, hour cycle, offset zone, parts,
  errors); negotiation and the current locale.
- `crates/svelters-host/tests/intl.rs` + `tests/js/intl.js`: about 55 JS-level checks through the
  host (for example `new Intl.NumberFormat('de-DE').format(1234.5) === '1.234,5'`, plural categories
  in en, pl and ru, `dateStyle` in en, de and fr, de sorting with ä, option errors). Then
  `Host::set_locale("de_DE.UTF-8")` and `("pl")` are seen by a JS subscriber, with the defaults
  following (`1.234,5`, then `1234,5` and `few`).
- `crates/svelters-host/tests/modules.rs` `a_locale_switch_re_renders_components_reading_locale`:
  a `.svelte` component reading `$locale` from `svelters:intl` re-renders from `en-US: 1,234.5` to
  `de-DE: 1.234,5` after `Host::set_locale`.
- `crates/svelters-egui/tests/intl.rs` (kittest): a `slider` under de-DE shows `2,50`, and typing
  `1,5` + Enter into its value box makes the value 1.5. After a switch to en-US the same widget
  shows `1.50` and reads `7.25`. A `drag-value` shows `1.234,5` and reads a typed `1,5`. There are
  also unit checks of `numeric::format`/`parse` in de, fr and en.

## 6. What was not run locally

The full workspace nextest, the full gate list and conformance were not run (GitHub CI is the gate).
Locally on macOS, only the tests listed in §5, filtered. Linux and Windows builds of the feature
were not run locally.

## 7. Deferred

- Named IANA time zones (needs tzdb offsets: `icu_time`'s zone offset data or jiff), and so
  `timeZoneName: "short"` showing `CET`.
- `Intl.RelativeTimeFormat` (QT-SURVEY lists it), `Intl.Locale`, `formatRange`.
- `inputmode="decimal"` on text kinds (QT-SURVEY §2.5), and `table` cells formatting numbers with
  the locale.
- A locale-aware `drag-value` commit mode (QW7) belongs to WIDGETS.
- More locales once cadatomic names them (regenerate the blob, or ship its own through `use_data`).

## 8. Handoff (IN1 agent stopped at its context limit, 2026-10-22)

- **Done and passing locally** (before the master merge): the svelters-intl unit tests (9) and
  `svelters-host --test intl`.
- **Written, not passed locally**: the egui kittest `tests/intl.rs` failed once on the slider's
  expected text (egui shows `2,50` for a 0..10 slider, not `2,5`). The expectations were fixed but
  not rerun. `drag_value_shows_and_reads_the_locale` (new, drag-value wired after INT1) and
  `modules.rs a_locale_switch_re_renders_components_reading_locale` (new, `svelters:intl`) have
  never run. A filtered run of all of these started after the merge of origin/master
  (`-E 'binary(intl) | test(a_locale_switch) | package(svelters-intl)'`); its result is in the
  report, if it finished.
- **Merge**: origin/master (INT1 becbeb89, DX1) is merged into ci/in1. Conflicts were resolved by
  keeping both sides, in `svelters-egui/Cargo.toml` (default features `dialogs` + `intl`),
  `svelters-egui/src/lib.rs` (`numeric` + menus/overlays) and `svelters-host/src/core.rs`
  (`locale` + `hmr_*` HostConfig fields). drag-value and range-slider are wired in
  `elements/inputs.rs`.
- **Next**: get CI green (the `svelters-intl` package is added to linux-gtk.yml's nextest lists);
  measure the size delta with the probe and record it in §3; check `svelters:intl` with svelte-check
  (`types/intl.d.ts`, `types/intl-module.d.ts`, new `./intl` export and the alias entries in
  packages/native-runtime/package.json).
- **Not run locally**: the full nextest, the gates, conformance, pnpm tests of native-runtime,
  Linux and Windows builds.
