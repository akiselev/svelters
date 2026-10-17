# FG1: one GTK backend build for GTK >= 4.14

Branch `ci/fix-gtk414`. Problem: on Ubuntu 24.04 (GTK 4.14) `svelters-gtk` did not link, because
the generated type table called `*_get_type` of kinds added after 4.14 (GtkSvg, GtkPopoverBin,
GtkTryExpression, GtkAccessibleHyperlink, all 4.22).

## Minimum: GTK 4.14 (with GLib 2.80)

- `MIN_GTK = (4, 14)` in `tools/metadata/gtk/gen_backend.py`, `compat::MIN` in
  `crates/svelters-gtk/src/compat.rs`. Keep them in sync.
- Going lower is not cheap: the backend links `girepository-2.0` (dynamic method calls, `gi.rs`),
  which is GLib >= 2.80. Distros with GLib 2.80 ship GTK >= 4.14 (Ubuntu 24.04: GTK 4.14.5, GLib
  2.80). The typed gtk-rs API is used with feature `v4_6` only, so gtk-rs itself is not the limit.

## How

- Metadata already carries GIR `since` (kinds: `availability.since`, properties: `since`). The
  generator emits `since: (major, minor)` on `KindInfo` and `PropInfo` when it is newer than the
  minimum, else `(0, 0)`. A property of a newer kind inherits the kind's version.
- `generated/types.rs`: kinds up to the minimum still link their `get_type` (so the link proves
  the minimum); newer kinds call `compat::call_get_type(c"gtk_x_get_type")`, which `dlsym`s the
  symbol in the library that provides `gtk_get_major_version` (the GTK actually loaded, works even
  if the backend is in an RTLD_LOCAL plugin), falling back to `RTLD_DEFAULT`.
- Run-time checks (`compat::require`) give one error format:
  `property GtkHeaderBar:use-native-controls needs GTK 4.18; the running GTK is 4.14.5`
  - `create`: kinds (`<svg> (GtkSvg) needs GTK 4.22; ...`);
  - `set_prop` / `get_prop`: properties;
  - `describe` (`SVL_PROP`): no descriptor, `last_error` says why.
  Kind/prop name resolution still succeeds, so the error is the version one, not "unknown".
- Methods go through GIRepository at run time (typelib lookup): a method missing from the
  running GTK's typelib is simply not resolvable. Signals are connected by name (GObject reports
  unknown signals). No typed gtk-rs call above `v4_6` exists in the crate.
- The GIR lacks a version for some members. Overrides can now set `since` on a property
  (`override.schema.json`); used for `GtkCalendar:date` (absent on 4.14.5; set to 4.16).

### Post-4.14 surface (from metadata `since`)

Kinds: GtkSvg, GtkPopoverBin, GtkTryExpression, GtkAccessibleHyperlink (4.22); enums/flags
GtkFontRendering, GtkTextBufferNotifyFlags (4.16), GtkInterfaceColorScheme/Contrast,
GtkWindowGravity (4.20), GtkReducedMotion, GtkSvgError/Features (4.22).
Properties: GraphicsOffload:black-background, Settings:gtk-font-rendering (4.16);
HeaderBar/WindowControls:use-native-controls, ListBox:tab-behavior, Widget:limit-events (4.18);
CssProvider:prefers-color-scheme/-contrast, Entry:menu-entry-icon-*-text,
FilterListModel:watch-items, Window:gravity, Settings:gtk-interface-color-scheme/-contrast (4.20);
CssProvider:prefers-reduced-motion, Picture:isolate-contents, SearchEntry:key-capture-widget,
Settings:gtk-interface-reduced-motion (4.22); ATContext:realized (4.24); Calendar:date (4.16,
override).

## Tests on old GTK

Skips print `SKIP: needs GTK x.y (running a.b.c)` (`testing::skip_below_gtk`) and return.

- Generated verify tests whose kind or property is newer than the minimum get the skip from the
  generator (e.g. `prop_headerbar_use_native_controls`, `prop_graphicsoffload_black_background`,
  `prop_calendar_date`).
- `counter::gallery` `gtk_07_bars` (4.18: `use-native-controls`) and `gtk_09_display` (4.16:
  `GraphicsOffload:black-background`), via `gallery_page!(..., needs (x, y))`.
- `style::token_changes_rewrite_one_provider_and_var_follows` (4.16: CSS custom properties,
  `:root` / `var()`), `style::the_scheme_drives_media_queries_in_every_provider` and
  `host_style::styles_from_svelte_...` (4.20: `@media (prefers-color-scheme)` and
  `GtkCssProvider:prefers-color-scheme`).
- `serdes::variants_dates_and_string_forms` asserts the version error for `Calendar:date` on < 4.16
  instead of the round trip.

Known limitation: on GTK < 4.16 style sheets that use tokens (`var(--sv-*)`) produce CSS parse
errors (reported through the parse-error log); the style layer does not lower tokens to literals.

## CI

`.github/workflows/linux-gtk.yml` job `gtk-4-14` (Ubuntu 24.04) is now required: builds bundles,
runs nextest for `svelters-gtk` and `counter` (gtk) under Xvfb. Needs `librsvg2-common` (GTK 4.14
loads SVG through gdk-pixbuf; 4.22 has its own loader), otherwise `prop_image_file` fails.

## Run log

- Run 1 (3776232): links and runs on 4.14.5. 606 passed, 8 failed: Calendar:date (no GIR
  version), image SVG loader, 2 gallery pages and 3 style tests (newer GTK features), serdes date.
  `ledger check` needed a refresh for the facts whose tests gained a skip.
- Run 2 (ed44830): Arch green; 4.14: 613 passed, 1 failed: `serdes` expected "<<nonsense" to be
  refused as a shortcut trigger, but GTK 4.14 parses it to an empty trigger (checked with PyGObject
  in an ubuntu:24.04 container). The test now uses `<Control>nosuchkey`, refused by both.
