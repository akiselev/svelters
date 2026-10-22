# AS1: asset imports

Design: `plans/research/ASSETS.md` §1–§3, with the user's decisions in §6 (AS-O1 (b): an `Asset` handle object;
AS-O2 (a); AS-O3 (a), the logical ids, is Q4's). Lane: AS1, branch `ci/as1`. Depends on Q1, H1/H3, WG1, WG2b
(all merged).

## What landed

### `svelters-modules::assets` (no toolkit types, `Send + Sync`)

- **`AssetRef`**: `asset://<root>/<logical path>`, validated and normalised (`new`, `parse`, `root`, `path`,
  `as_uri`, `mime`); `FromStr`, `TryFrom<String>`, `Into<String>`; with feature `serde` (on with `source`, and
  in svelters-host) `Serialize`/`Deserialize` as the string and `JsonSchema` `{ type: string, format:
  "svelters-asset" }`. Roots are `[a-z0-9._-]+`, `svelters` reserved; no `..` out of the root, no `?`/`#`.
- **`Assets`** (`Arc`): `mount(root, AssetSource::{Dir, Static})` (a root mounts once), `unmount`, `register(root,
  file)` (an import's file: its path in the root, or `node_modules/<package name>/<path in package>` from the nearest
  package.json for a dependency), `bytes` (`AssetBytes::{Static, Shared}`), `info` (`mime`, `len`, content `hash`,
  `version`), `version`, `file_path` (confined to the root, symlinks resolved), `on_change(f) -> Subscription`,
  `watch_files`, `serves(path)`, `changed(paths)` (bumps versions, fires the sinks once), `notify`.
  `data_url(mime, bytes)`; `mime_of` (§1.1's table).
- **`ModuleProvider::assets() -> Option<Arc<Assets>>`** (default `None`). `SourceProvider` owns one with `app`
  mounted at the app root (`SourceProvider::assets()`); `BundleProvider` mounts `<bundle dir>/assets` as `app`
  (what the Vite plugin writes); `ComposedProvider` forwards. The host and the egui backend read the provider's
  registry, so nobody wires one through by hand.
- **`svelters:assets`** (`assets.js`, served by `ComposedProvider`, always visible like `svelters:core`):
  `class Asset` (`url`, `root`, `path`, `mime`, `size`, `info()`, `text()`, `bytes()`, `onChange(f)`,
  `toString`/`toJSON`/`Symbol.toPrimitive` give the ref), `asset(refOrPath)` (one handle per ref; a bare path is
  under the app root), `text`, `bytes`, `isAsset`. The ref is a hidden non-enumerable own property
  `__svelters_asset` (`assets::HANDLE_KEY`).

### Imports (`SourceProvider`; the old `export default "<absolute path>"` is gone)

| Import | Module | Value | Its file changes |
|---|---|---|---|
| `./icon.svg` (any asset extension), `./x?url` | ref: `export default asset("asset://app/src/icon.svg")` | the `Asset` handle | no module runs; `onChange` listeners run |
| `./x?raw` | `export default text(ref)` | string | H1 re-evaluates it (it is in `modules_at`) |
| `./x?bytes` | `export default bytes(ref)` | `Uint8Array` | H1 |
| `./x?inline` | `export default "data:<mime>;base64,…"` (built in Rust) | string | H1 |
| `./x?worker` etc. | resolve error naming the supported suffixes | | |

Ref modules are graph nodes (for `importers`) flagged `asset_ref`: `invalidate` and `modules_at` skip them.
`transform.rs` `Kind::Asset` no longer makes code (a `check` of an asset file is empty).

### `svelters-host`

- **Handle → ref at every native boundary:** `convert.rs` `encode_at` turns an object with `__svelters_asset` into
  that string before matching the slot (`JsReader::asset_ref`), so a handle works in any string prop
  (`image.source`, `icon.source`, `tooltip-text`), an export argument, a `Json<T>` field and nested values.
- **Ops:** `__svelters_host.assets(op, ref)` (`text`, `info`) and `assetBytes(ref)` (an `ArrayBuffer`, one copy),
  through `Core::assets`, set from `provider.assets()` at `load_bundle`/`load_entry`.
- **Reload partition (`reload.rs`):** changed paths go to `assets.changed` first. A path that is only a served
  asset is consumed there (never reaches `invalidate`); one that is also a value module goes on to H1. The
  `Asset.onChange` listeners run (`JsEngine::assets_changed`, jobs drained). An asset-only change returns
  `ReloadReport { ok: true, hot: Some([]), assets: [refs] }`, so it works in embed mode too. The watch set is
  `provider.watch_files() ∪ assets.watch_files()`.
- **`AssetRef` as an export type:** `JsType`/`FromJs`/`IntoJs` (TS `import('@svelters/native-runtime/assets').Asset`;
  a non-ref string is a `type` `NativeError`); a result is wrapped into the handle when `svelters:assets` is loaded
  (`globalThis.__svelters_asset`). `Json<T>` fields with format `svelters-asset` type as `Asset | string`.
  `svelters::AssetRef`/`svelters::Assets` re-exported.

### `packages/native-runtime`

- `types/assets.d.ts` (`@svelters/native-runtime/assets`: `class Asset`) + `types/asset-imports.d.ts` (the
  wildcard modules for §1.1's extensions, `?url`, `?raw`, `?inline`, `?bytes`, and `svelters:assets`).
  `check/assets.ts` type-checks them.
- `vite.js` `sveltersAssets(root)` (in `nativeConfig`'s plugins, `enforce: 'pre'`): asset files and `?url` give
  the same ref module as the Rust loader; `?bytes` the same value module; `?raw`/`?inline` stay Vite's;
  `svelters:assets` is external (the host serves it); `vite build` emits every referenced asset at
  `assets/<logical path>`. The alias table stays the one `package.json` table both sides read.

### `svelters-egui::assets`

- `install(&Context, Arc<Assets>)` (idempotent per context; a second registry is served too, the first that has
  the ref answers): an `asset://` + `data:` (base64 or percent-encoded) `BytesLoader`; an own SVG `ImageLoader` on
  `resvg =0.45.1` (default features off, the version egui_extras 0.37 pins) that rasterises at egui's size hint,
  caches per (uri, hint), styles `svg { color: #fff }` so `currentColor` icons tint like glyphs, and refuses every
  `href` except `data:`; WG2b's PNG loader reaches `asset://x.png` through `try_load_bytes`, so `image.source`
  needed no change.
- An `Assets::on_change` sink: `forget_image(uri)`, queue the uri, `request_repaint`; the next logic pass
  (`frame.rs`) marks `image`/`icon` nodes showing it for measure. No JS runs.
- `image(&AssetRef)`, `source(&AssetRef) -> ImageSource` (instead of `From`: the orphan rule), `color_image(&Assets,
  &AssetRef, [w, h])`, `texture(&Context, &AssetRef, Vec2)` cached by (uri, version, pixel size).
- `icon` gains `source: string | null` and `tint: bool` (default true): a square of `size` (else body size), the
  image fitted inside and tinted with the text/state colour; WG1b's placeholder box when it does not load.
  `metadata/semantics/egui.json` and `types/egui.d.ts` regenerated (`SVELTERS_UPDATE=1 … --test spec`).
- `Embed::assets()`: the registry of the provider `load_from`/`load_app_from` loaded; the loaders are installed on
  the embed's context and every `attach`ed one (the runners load through `load_app_from`).
- `svelters-modules` is now a non-optional dependency of svelters-egui (base crate; `source` adds the compiler).

## Tests

- `svelters-modules` `assets::tests::*` (refs, base64, dir roots with versions/sinks/confinement, dependency
  paths, static roots), `transform::tests::json_and_assets`.
- `svelters-host` `modules::asset_imports_are_handles_and_their_changes_run_no_modules`: handle identity
  (`./icon.svg` = `?url` = `asset('src/icon.svg')`), `?raw`/`?inline`/`?bytes` values, `JSON.stringify`, the handle
  reaching a native prop as its ref, an asset change → `hot: Some([])`, `assets: [ref]`, `onChange` ran, the app
  module evaluated once; ref modules not in `modules_at`/`invalidate`, the `?raw` ones are; `?worker` refused.
  `an_app_loads_from_source_with_json_assets_and_runes_modules` updated (the handle, `text()`).
- `svelters` `json::asset_refs_round_trip`: `AssetRef` parameter (handle or string), result (handle), `Json<T>`
  field, a bad string refused, the typing.
- `native-runtime` `test/assets.test.mjs`: the Vite plugin's modules and emitted files; the `Asset` handle.

- `svelters-egui` `tests/assets.rs`: `bytes_loader_serves_asset_and_data_uris`, `svg_ignores_external_hrefs`,
  `icon_with_a_source_is_measured_square_and_loads_the_asset`, `changing_the_file_forgets_the_image_and_relayouts_with_no_js`
  (SVG 16 → 32 px), `texture_is_cached_by_version_and_size`, and `imported_svg_updates_on_edit_without_running_js`
  (a Svelte component from source in an `Embed`: `import icon from './icon.svg'`, `<icon source={icon}/>`, edit the
  file, `apply_changes` → `hot: Some([])`, one asset, the image 16 → 32 px). No pixel check of the tint (kittest
  without a renderer).

## Not done (left for later lanes)

- `with { type: 'bytes' }` on the Rust loader: the QuickJS resolver gets no import attributes, so only `?bytes`
  works there (the Vite plugin takes both).
- `asset(path)` resolves under the `app` root, not the importer's package root (extensions' roots are AS3).
- Exports returning an `AssetRef` give the handle only once `svelters:assets` is loaded somewhere (else the string);
  `Json<T>` results carry refs as strings.
- `Overlay` edits of asset text (the live IDE editing an SVG): the overlay is not read by `Assets` yet.
- Fonts and CSS `url()` (AS2), the `.svb` bundle and `AssetSource::Bundle` (Q4), extension roots (AS3).
- The egui sink holds a `Context` clone and the context holds the `Subscription` (a cycle for the context's
  lifetime, which is the process's).
- GTK/Qt/AppKit/WinUI image props taking refs (`Assets::file_path` exists; maintenance-level).
