# ASSETS: imported resources through the module loader, hot in dev, bundled in release

Status: architecture proposal (research, no code, nothing built or run). 2026-10-22. svelters master `87eaf567`;
branches `ci/h1` `0cd77272`, `ci/h3` `270e41aa`, `ci/wg1` `9449c9ae`, `ci/wg2b` `c2f077e5`, `ci/wg4b` `f90ea71e`;
cadatomic master `d2a5789`. Answers the user's request (2026-10-22): "lets investigate using the module loader for
resources. Users should be able to import svg and other assets and pass them to egui/rust, like extensions loading
their sidebar icon. In dev this should hot reload from the file system, then in production bundle them in (along
with bundle source, precompiled quickjs bytecode, etc)".

Refines: QUICKJS-RUNTIME §3.2 "Assets" row and §7 Q3 (the asset model), §3.5 (release R1); TRACKS LOADER Q2/Q4;
Q1 (`transform.rs` `Kind::Asset`, "Assets resolve to absolute paths, which is dev only"); H1/H3 (`reload.rs`,
`modules_at`); WG1 `icon`, WG2b `image` (`elements/images.rs`), CS5e (`style.rs`). Leaves room for ARCH-SECURITY
(deferred) and builds nothing for it.

## Binding inputs

- **Decision 5 (user, 2026-10-21):** svelters' own Rust module loader; live compile of `.svelte`/`.svelte.js` in
  release builds too; the hot-reload path with `$state` kept; Vite is a dev convenience only.
- **egui 0.37**, upgraded together with cadatomic. egui_extras is pinned `=0.37.0` (WG4b), which pins `resvg 0.45.1`.
- **ARCH-SECURITY deferred** (user, 2026-10-22): "leave room for security policies later but right now we're
  developing". Prototype stance: the simple design, with room.
- **cadatomic** (read-only): extensions run in isolates off the UI thread (framework §5.1); the shell is
  manifest-first, "declare first, load later" (native-ext §5.3, §6.3), so an extension's sidebar icon must show
  before any of its code runs; first-party Svelte ships as precompiled bytecode in release and the IDE edits it as
  overlays recording a base content hash (js-api §6.3, question J6); cadatomic's icons today are Phosphor glyphs in
  an `icons` font family (`crates/cadatomic-app/src/theme/icons.rs`), drawn through WG1's `icon`.

## What exists (verified in code)

| Piece | Where | Fact |
|---|---|---|
| Asset import | `svelters-modules/src/source/transform.rs` `Kind::Asset`, `asset_module` | any extension not js/json/svelte/ts/cjs/css is an asset: `export default "<absolute path>"`. No query suffixes; specifiers with `?` fail to resolve. The asset is a graph node, so editing it reloads the app (`invalidate`) or, under H1, swaps its importers |
| Image | `ci/wg2b` `elements/images.rs` | `image.source`: bare path, `file://`, `bytes://`, or any uri an installed egui loader takes. Own `PngLoader` (PNG only, `image` crate), cached per uri, scaled to `max_texture_side`. Non-PNG bytes → `NotSupported`, so other loaders are asked |
| Icon | `ci/wg1` `fonts.rs`, `kinds/decor.rs` | `icon` = a glyph (`name` via `register_icons` table, or `glyph`) in a font family. No image source |
| CSS | `svelters-egui/src/style.rs` (CS5e) | engine props: colours, borders, opacity, font size, grid, margins, padding. No `background-image`, no `url()` |
| DnD | `ci/wg4b` `kinds/dnd.rs` | payloads are strings; nothing asset-specific |
| egui loaders | egui 0.37 `load.rs`, `context.rs` | `BytesLoader` (`load(ctx, uri)`, `forget`), `ImageLoader`, `Bytes::{Static, Shared}`, `Context::forget_image(uri)` clears every loader's entry for a uri; `has_extension(uri, "svg")` splits on `#` only, so a `?v=` query breaks extension sniffing. `Image` asks loaders with a `SizeHint` already scaled by `pixels_per_point` |
| SVG | egui_extras 0.37 `loaders/svg_loader.rs`, `image.rs` | `SvgLoader` caches per (uri, size hint), default `usvg::Options` (no per-uri style sheet). `load_svg_bytes_with_size` is public. usvg 0.45.1 `Options::style_sheet` and `image_href_resolver` exist |
| Fonts | egui 0.37 | `Context::add_font(FontInsert)`, `set_fonts` |
| Reload | `ci/h3` `svelters-host/src/reload.rs` | `invalidate(changed)` → `modules_at` → `hot_update`; in embed mode a change that cannot apply in place is an error, not a reload |

## 0. Summary

- **An import gives an opaque, stable string:** `import icon from './icon.svg'` → `"asset://app/src/icon.svg"`, typed
  as a branded `Asset` string. It names a logical path, not content, so it is the same in dev and release and does
  not change when the file does. `?raw`, `?inline`, `?bytes` (and `with { type: 'bytes' }`) give values.
- **One Rust registry, `Assets`** (`Arc`, `Send + Sync`, in `svelters-modules`, no egui): roots (`app`, `ext.<id>`)
  mounted from a directory, a bundle or static bytes; `bytes`, `info`, `on_change`. `AssetRef` is the typed form of
  the string and crosses every boundary as a string (props, RA1 kind props, `Json<T>`, isolates).
- **egui serves `asset://` through its own loader chain:** a svelters `BytesLoader` for `asset://` (and `data:`), an
  SVG `ImageLoader` on resvg 0.45.1 (tintable monochrome icons), WG2b's PNG loader. `image.source` and a new
  `icon.source` take refs; cadatomic's Rust panels write `egui::Image::new(&r)`.
- **Dev reload touches no JS:** the watcher sees `icon.svg`, `Assets` bumps its version, egui forgets the uri and
  repaints, nodes showing it remeasure. Only value imports (`?raw`, `?bytes`, …) go through H1.
- **Release: one container, `.svb`** (an index plus content-addressed blobs) holding per module the source
  (optional), the compiled JS, the map and optionally QuickJS bytecode with an engine stamp; the assets (with
  optional pre-rasterised SVG); the package manifests. It replaces Q4's `app.qbc`. `include_bytes!` or a file.
- **Layers:** source overlay (live IDE) → dev directories → bundle, read through one layer stack by the same
  `SourceProvider`, with logical module ids, so an overlay on a release bundle is exactly a dev edit.
- **Extensions use the same format**, mounted as root `ext.<id>`, confined to it; the shell reads the manifest and
  shows the icon from the bundle without running JS.
- **Four lanes:** AS1 (imports, registry, egui consumers, dev reload), AS2 (fonts, CSS `url()`), Q4 extended (the
  bundle), AS3 (extension bundles). SC1 needs none of them; an extension's sidebar icon needs AS1 (AS3 when packaged).

## 1. Import surface

### 1.1 What an import gives

| Import | Default export | Module kind | On file change |
|---|---|---|---|
| `import i from './icon.svg'` (any asset extension) | `Asset`: `"asset://app/src/icon.svg"` | ref (generated, constant) | no JS runs (§3) |
| `'./icon.svg?url'` | same `Asset` (Vite parity: `?url` is the default for assets) | ref | no JS runs |
| `'./shader.wgsl?raw'` | `string` (UTF-8 text) | value | H1 re-evaluates it and its importers up to a boundary |
| `'./icon.svg?inline'` | `string`: `data:<mime>;base64,…` (Vite parity) | value | H1 |
| `'./mesh.bin?bytes'`, `import b from './mesh.bin' with { type: 'bytes' }` | `Uint8Array` | value | H1 |
| `'./data.json'`, `with { type: 'json' }` | the parsed JSON (Q1, unchanged) | value | H1 |
| `'./data.json?url'` | `Asset` | ref | no JS runs |
| `'./x.css'`, `?worker`, `?sharedworker`, `.wasm` | load error with a hint (Q1's CSS message stays) | — | — |

- **Ref modules** are generated: `export default "asset://app/src/icon.svg";`. The loader also registers the file
  with `Assets` (watch list, §3). The graph keeps the edge for `importers` but `modules_at` does not report a ref
  module, so H1 never sees it.
- **Value modules** are small generated modules calling host ops, so a large file never becomes JS text:
  `import { text } from 'svelters:assets'; export default text("asset://app/src/a.wgsl");` (`bytes(..)` likewise,
  an `ArrayBuffer` copied once from Rust). `?inline` builds the data URL in Rust. `svelters:assets` is a native
  module served through LD1's `NativeRegistry`, always visible like `svelters:core`.
- **Dynamic names** (`icons/${name}.svg`): `import { asset } from 'svelters:assets'; asset('icons/gear.svg')`
  returns the ref for a path under the importer's package root. It is the only non-import route, and the bundler
  includes such files through the package's include globs (§4.2). `import.meta.glob` and
  `new URL(.., import.meta.url)` are not planned (QuickJS has no `URL`).
- **Which extensions are assets:** Q1's rule stays (everything that is not js/json/svelte/ts/cjs/css). The MIME type
  comes from the extension (a fixed table: svg, png, jpg/jpeg, gif, webp, ico, ttf, otf, woff2, txt, bin;
  otherwise `application/octet-stream`).

### 1.2 The ref string

`asset://<root>/<logical path>`:

- `<root>` is a mounted root: `app` for the primary app, `ext.<id>` for an extension (`ext.acme.gerber`), names an
  embedder chooses for its own mounts (cadatomic: `cad`). `[a-z0-9._-]+`, `svelters` reserved.
- `<logical path>` is relative to the root's package directory, `/`-separated, normalised, no `..`. A file in a
  dependency is `node_modules/<package name>/<path in package>` (the install path, not pnpm's real path), found from
  the nearest `package.json` above the canonical file. Same rule in dev, in the Vite plugin and in the bundle.
- **Why a string:** it crosses the C ABI as `SVL_STRING` with no new value type; it is valid in `image.source`
  (an egui uri), in `drag-data`, in `JSON.stringify`, in `Json<T>`, across an isolate's op batches and in manifests;
  it reads well in logs and devtools. A handle object would need a host object table entry per asset, a lifetime,
  and a conversion at every boundary (RA1 notes that object handles in kind props do not convert yet).
- **Why not content-hashed:** a content hash would change the JS value on every edit, so every importer would
  re-run. A path ref is stable; the content version lives in Rust (§3). The release bundle content-addresses the
  bytes behind the path (§4).
- **Why `asset://`, not `asset:`:** egui loaders and `has_extension` work on it unchanged (`Path::extension` of
  `asset://app/x.svg` is `svg`); the version is never in the uri (`?v=` would break SVG sniffing).

### 1.3 TypeScript

`packages/native-runtime/types/assets.d.ts` (exported as `@svelters/native-runtime/assets`, referenced from the app
template's `tsconfig`, replacing `vite/client`'s asset declarations, which declare the same patterns as `string`):

```ts
declare const asset: unique symbol;
export type Asset = string & { readonly [asset]: true };   // assignable to string, not the reverse

declare module '*.svg'  { const ref: import('@svelters/native-runtime/assets').Asset; export default ref; }
// … png, jpg, jpeg, gif, webp, ico, ttf, otf, woff2, bin, txt: the same
declare module '*?url'    { const ref: import('@svelters/native-runtime/assets').Asset; export default ref; }
declare module '*?raw'    { const text: string; export default text; }
declare module '*?inline' { const dataUrl: string; export default dataUrl; }
declare module '*?bytes'  { const bytes: Uint8Array; export default bytes; }

// svelters:assets
export function asset(path: string): Asset;
export function text(ref: Asset): string;
export function bytes(ref: Asset): Uint8Array;
```

`egui.d.ts` (generated): `image.source: Asset | string`, `icon.source: Asset | string | null`. Rust `AssetRef` props
and `Json<T>` fields generate `Asset` (§2.2).

### 1.4 Parity on the Vite path (non-egui backends, Vite apps)

`packages/native-runtime/vite.js` gets a `svelters:assets` plugin (`enforce: 'pre'`): `resolveId`/`load` for asset
extensions and `?url` return the same ref module; `?bytes` and `with { type: 'bytes' }` the same value module;
`?raw`/`?inline` stay Vite's (same values). `vite build` copies every referenced asset (plus the include globs) to
`dist/assets/<logical path>`; the runner mounts `dist/assets` as root `app` when it loads `dist/app.js` through
`BundleProvider`. So the JS value of an import is identical under Vite, the Rust loader and a release bundle.
GTK/Qt/AppKit/WinUI image props that need a file call `Assets::file_path(ref)` (a dir root returns the real path;
a bundle root extracts once to a cache directory). Maintenance-level, one call per image kind, not scheduled.

## 2. Rust side

### 2.1 Types (`svelters-modules::assets`; no egui, `Send + Sync`)

```rust
/// "asset://<root>/<path>". Serialises as the string; JsonSchema `{ type: string, format: "svelters-asset" }`.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AssetRef(Arc<str>);
impl AssetRef {
    pub fn new(root: &str, path: &str) -> Result<AssetRef, AssetError>;   // validates, normalises
    pub fn parse(uri: &str) -> Result<AssetRef, AssetError>;
    pub fn root(&self) -> &str;
    pub fn path(&self) -> &str;
    pub fn as_uri(&self) -> &str;
}

pub enum AssetBytes { Static(&'static [u8]), Shared(Arc<[u8]>) }   // maps 1:1 onto egui::load::Bytes

pub enum AssetSource {
    Dir(PathBuf),                         // dev: a package directory, watched
    Bundle(Arc<Bundle>),                  // release, extensions (§4)
    Static(&'static [(&'static str, &'static [u8])]),   // tests, tiny embeds
}

pub struct AssetInfo { pub mime: &'static str, pub len: u64, pub hash: ContentHash, pub version: u64 }

pub struct Assets { /* RwLock<roots>, per-ref version and hash, served-path watch list, sinks */ }
impl Assets {
    pub fn new() -> Arc<Assets>;
    pub fn mount(&self, root: &str, src: AssetSource) -> Result<(), AssetError>;   // a root mounts once
    pub fn unmount(&self, root: &str);
    pub fn bytes(&self, r: &AssetRef) -> Result<AssetBytes, AssetError>;
    pub fn info(&self, r: &AssetRef) -> Result<AssetInfo, AssetError>;
    pub fn file_path(&self, r: &AssetRef) -> Option<PathBuf>;
    pub fn on_change(&self, f: impl Fn(&[AssetRef]) + Send + Sync + 'static) -> Subscription;
    // used by the loader and the reloader:
    pub fn register(&self, root: &str, file: &Path) -> Result<AssetRef, AssetError>;   // an import outside the root (node_modules)
    pub fn watch_files(&self) -> Vec<PathBuf>;
    pub fn changed(&self, paths: &[PathBuf]) -> Vec<AssetRef>;   // bumps versions, fires sinks
}
```

- **Confinement is by root,** as `BundleProvider` and Q1's `confine` already do: a ref's path never leaves its root
  (canonicalised, symlinks re-checked); files outside the root are reachable only after the loader `register`ed
  them from an import the resolver allowed. A root records an owner slot (`Option<PrincipalId>`, `None` today) so
  SEC1/SEC2 can later check "a root of principal P may show only refs of roots P may read" at `set_prop`; nothing
  checks it now.
- **The host owns one `Assets`** (`HostConfig::assets`, default a new one with `app` mounted at the app root) and
  passes it to `SourceProvider` (refs, `register`) and to the backend. Isolates share it (`Arc`): an extension's
  `import icon from './icon.svg'` yields a string that reaches the UI thread's `image` node in an op batch, and the
  UI thread's loader reads the same registry.

### 2.2 Props and exports

- **RA1 kind props:** `#[prop] icon: Option<AssetRef>`. `FromJs` parses (a bad string is refused like a bad enum
  value), `IntoJs` writes the string; the `JsType` is a string with TS alias `Asset`.
- **RA2 `Json<T>`:** `AssetRef` is `Serialize + Deserialize + JsonSchema` with `format: "svelters-asset"`;
  `json::schema_ts` maps that format to `Asset`. Exports returning an `AssetRef` give JS the string.
- **Built-in egui props:** `image.source` accepts refs as egui uris (no prop change, only the loader). WG1's `icon`
  gains `source: string | null` (an image instead of a glyph) and `tint: bool` (default true for SVG): §2.3.

### 2.3 egui (`svelters-egui::assets`)

```rust
pub fn install(ctx: &egui::Context, assets: Arc<Assets>);   // once per context; Embed and the runner call it
pub fn image(r: &AssetRef) -> egui::Image<'static>;         // egui::Image::new(r.as_uri())
impl From<&AssetRef> for egui::ImageSource<'static>;
pub fn texture(ctx: &egui::Context, r: &AssetRef, size: egui::Vec2) -> Result<egui::TextureHandle, AssetError>;
pub fn color_image(assets: &Assets, r: &AssetRef, px: [u32; 2]) -> Result<egui::ColorImage, AssetError>;
impl Embed { pub fn assets(&self) -> &Arc<Assets>; }
```

- **Loaders `install` adds:**
  1. `AssetBytesLoader` (`egui::load::BytesLoader`): `asset://` → `Assets::bytes` (zero-copy `Bytes::Static` from an
     `include_bytes!` bundle) with the MIME type; `data:` (base64) for `?inline` values.
  2. `SvgLoader` (own, ~100 lines, `resvg = "=0.45.1"`, default features off: the same copy egui_extras pins, so
     cadatomic links one resvg): rasterises at the `SizeHint` egui's `Image` computes (already × `pixels_per_point`),
     cached per (uri, hint) and cleared by `forget`. Not egui_extras' `SvgLoader`, because it cannot set per-load
     options: ours sets `style_sheet = "svg { color: #fff }"` so `currentColor` icons render white and tint to the
     node's text colour (`Image::tint`), like a glyph; and its `image_href_resolver` refuses everything but `data:`
     (a bundle has no file system, and an SVG must not read files).
  3. WG2b's `PngLoader` stays, already reached for non-file uris through `try_load_bytes`.
  Other formats (JPEG, GIF, WebP) stay the embedder's (egui_extras `image`/`gif`/`webp` loaders) as WG2b decided.
- **`icon source`:** measured at `size` (else body size) square, drawn with `egui::Image` fitted inside, tinted
  with the computed `color` unless `tint=false`; selected/hover/disabled states follow the same visuals as a glyph
  icon. The fallback box of WG1b covers a source that does not load. This is how an extension's sidebar icon looks
  identical to cadatomic's Phosphor glyphs.
- **Rust panels** (cadatomic): `ui.add(egui::Image::new(&r).fit_to_exact_size(vec2(16., 16.)).tint(fg))`. egui's
  cache and §3's invalidation apply with no extra code. `texture()` is for code that wants a `TextureHandle`
  (cached by (ref, version, pixel size)); `on_change` for anything else it derived.

### 2.4 Fonts (AS2)

- Rust: `fonts::register_font(&assets, &r, FontSpec { name, families: [(FontFamily, Position)], weight })`, which
  reads the bytes and calls `Context::add_font`; WG1's `register_weight`/`register_icons` then work on the name.
- JS: `registerFont(ref, { family, weight })` and `registerIcons(family, names)` in `svelters:app/egui`, so an
  extension can ship an icon font. A font asset change re-adds the font data (fonts reload with no JS run).

### 2.5 CSS `url()` (AS2)

Where a component's sheet text is produced (the loader's style step for egui from source; `vite.js` `nativeSheets`),
every relative `url(...)` is resolved against the component's file and rewritten to `url("asset://…")`, registering
the asset; absolute and `data:` urls stay; `http(s):` urls are refused with a warning (no fetch from stylesheets,
ARCH-SECURITY §5's Q2/Q3 row). The egui engine gains `background-image` (`url()` or `none`) plus `background-size`
(`contain`/`cover`/`auto`, through WG2b's `fit_rect`), painted between the background colour and the content.

## 3. Dev hot reload

```
watcher ─ changed paths ─▶ reload.rs checkpoint
                             ├─ assets.changed(paths) ──▶ versions bumped ──▶ sinks:
                             │                             egui: ctx.forget_image(uri) per ref, Dirty::LAYOUT on
                             │                                   nodes whose source is a changed ref, request_repaint
                             │                             fonts: add_font again (AS2)
                             │                             embedder: on_change (cadatomic textures)
                             └─ the rest ─▶ provider.invalidate / modules_at ─▶ H1 hot_update or full reload
```

- **Watch set:** `provider.watch_files() ∪ assets.watch_files()`. `Assets` watches every file it served from a `Dir`
  root (imports, `asset()`, CSS urls, Rust reads, manifest icons).
- **Partition first:** a path that is only a served asset is consumed by `assets.changed` and never reaches
  `invalidate`/`modules_at`. A path that is also a value module (`?raw`, `?bytes`, `?inline`) goes to both: its
  value module is re-evaluated by H1 and propagates to the first accepting importer (H3 keeps `$state`).
- **No JS runs for a ref import**, so the asset path also works in **embed mode** (cadatomic), where H1 turns any
  change it cannot apply in place into an error rather than a reload. `ReloadReport` gains `assets: Vec<AssetRef>`;
  a report with only assets has `hot: Some([])`, `ok: true`.
- **Size changes:** an `image`/`icon` whose intrinsic size depends on the asset is marked for layout, so a resized SVG
  relayouts in the same frame (WG2b already remeasures when a loader finishes).
- **Overlay:** the live IDE edits an SVG as text. `Overlay::set(path, text)` covers asset reads too (layers, §4.3);
  `Host::apply_changes([path])` then runs the same partition.
- Rejected: content-versioned refs (`asset://…#<hash>`) that re-run importers through H1. They would re-run
  components (and their effects) for a pixel change and do nothing in embed mode.

## 4. Production bundle

### 4.1 The container: `.svb`

One format for the app and for extensions. It replaces Q4's `app.qbc` (the bytecode table is one of its sections).

```
"SVB1" | u32 index_len | index (JSON) | blob area (each blob 16-byte aligned)

index = {
  format: 1, kind: "app" | "extension", root: "app" | "ext.<id>", version, build: "<content hash of the index>",
  stamp:  { compiler: "<svelters-compiler version>", runtime: "<@svelters/native-runtime version>",
            engine: "quickjs-ng", engine_build: "<rquickjs-sys version>+<options hash>" | null },
  entry:  { kind: "app" | "embed" | "module", names: {…} },
  files:  { "<logical path>": { blob, hash, codec: "none" | "deflate", role: "source" | "manifest" | "asset", mime? } },
  modules: { "<logical id>": { source?: "<logical path>", js: blob, map?: blob, bytecode?: blob,
                               source_hash, imports: { "<specifier>": "<logical id>" } } },
  rasters: { "<logical path>": [ { px: [w, h], blob } ] },        // optional pre-rasterised SVG
  include: [ "<glob>" ], externals: [ "svelte", "@svelters/native-runtime", … ],   // extension bundles
  meta: { … }                                                     // embedder data, opaque (cadatomic's manifest name)
}
```

- **Content-addressed blobs** (BLAKE3: new to svelters, already in cadatomic's lockfile): identical files dedupe; `hash` is the base
  content hash cadatomic's overlays record (js-api §6.3).
- **Per module, three tiers:** `source` (the `.svelte`/`.svelte.js`/`.js`, optional), `js` (the compiler's output,
  always, so startup never runs the Svelte compiler), `bytecode` (optional; valid only when `stamp.engine_build`
  equals the running engine's, else ignored and the JS is used). `imports` is Q4's R1 specifier table, so startup
  resolves nothing.
- **Package manifests** (`package.json` of the app and of each dependency) are stored as files, so live compile in
  release can resolve a new import through the same resolver (§4.3).
- **Compression:** `deflate` (`miniz_oxide`, already in the lockfile) for sources, JS and maps; assets, bytecode and
  rasters stored raw (zero-copy `&'static [u8]` from `include_bytes!`). Sizes are measured in Q4.
- **Why not zip:** zero-copy static slices, alignment and a JSON index are ~200 lines with no dependency; zip adds a
  crate for nothing we use. A `svelters-bundle ls|cat|extract` subcommand covers inspection.

### 4.2 Building it

- `svelters_modules::bundle::build(cfg) -> Bundle` (a library function, pure Rust: resolver + compiler, no QuickJS),
  so it runs from a `build.rs` before `include_bytes!`, and as `svelters-bundle build <app dir> -o app.svb`.
- **Contents:** the module graph from the entry (production condition, `dev: false`, no `--hmr`); every asset the
  graph or the CSS references; the package's `"svelters": { "assets": ["icons/**", …] }` globs (dynamic `asset()`
  names, manifest-referenced files); `sources: true|false` (decision 5: true by default; cadatomic may ship without
  and fetch by `build` hash, J6); `rasters`: optional `[(path, [sizes])]` for SVGs whose first frame matters.
- **Bytecode is written by the engine that will read it:** `svelters-bundle precompile in.svb -o out.svb` runs inside
  the target binary (`Host::precompile_bundle`, a hidden runner flag), compiles each module's `js` with the linked
  QuickJS and fills `bytecode` and `stamp.engine_build`. A `build.rs` cannot do this safely (a build-dependency
  QuickJS may differ in features or target). Without that step, Q2's on-disk bytecode cache fills on first launch
  (probe: 95–120 ms cold, ~13 ms warm for 115 modules, QUICKJS-RUNTIME §3.2).

### 4.3 Serving: one provider over a layer stack

`SourceProvider` reads every file (sources, manifests, assets) through a layer stack instead of `std::fs`:

```
Overlay (live IDE buffers; per principal later)          ← highest
Dir layers (dev: the app dir; cadatomic's user/project layer trees, mapped to logical paths by cadatomic)
Bundle layer(s)                                          ← lowest; read-only
```

```rust
pub trait Layer: Send + Sync {
    fn read(&self, logical: &str) -> Option<Result<AssetBytes, String>>;   // None: not in this layer
    fn kind(&self, logical: &str) -> Option<FileKind>;                     // file | dir, for the resolver
    fn real_path(&self, logical: &str) -> Option<PathBuf>;                 // for the watcher and file_path
}
```

- **Module ids become logical** (`app/src/Panel.svelte`, `app/node_modules/svelte/src/…`) instead of absolute
  paths, so the same id names a module whether it comes from disk, an overlay or the bundle. H1's versions, H3's
  registry keys, ID1's identity and source maps then agree across dev, release and the live IDE. The `Dir` layer
  maps ids to real paths (editors and stack traces get the real path through `real_path`).
- **Per module:** take the highest layer that has the source. If that is the bundle and its `source_hash` matches,
  serve `bytecode` (`ModuleCode::Cached`) when the stamp matches, else `js` (+ map); resolve through the `imports`
  table. Otherwise (an overlay or dir edit) compile with the linked compiler (decision 5) and resolve with the
  resolver over the layer stack. Bytecode of compiled modules goes to Q2's cache.
- **Assets** go through the same stack: an `AssetSource::Bundle` root is a bundle layer under any dir/overlay layer
  for that root. So an SVG edited in the live IDE overrides the shipped one with no special case.
- **No file system needed:** an app whose only layer is an embedded bundle never calls `std::fs` (bytecode cache
  aside, which an embedder can turn off).
- **Embedding:** `Bundle::from_static(include_bytes!(concat!(env!("OUT_DIR"), "/app.svb")))` (one binary, the
  stamp always matches; cadatomic's default) or `Bundle::open(path)` (memory-mapped with `memmap2`, already in the
  lockfile; for extensions and for apps that update the UI separately). Same type either way.
- `BundleProvider` (a Vite `dist/` directory) stays for the Vite path.

### 4.4 Extension bundles (AS3)

- Same `.svb`, `kind: "extension"`, `root: "ext.<id>"`. The shell `Assets::mount("ext.<id>", Bundle(..))` at
  discovery; the manifest (`meta` names it, e.g. `cadatomic.json`) is a plain `files` entry, readable with
  `Bundle::file(path)` **without running JS**, so the sidebar shows
  `egui::Image::new(&AssetRef::new("ext.acme", "icons/sidebar.svg")?)` before activation.
- **Confined:** its modules and assets resolve only inside its root; `externals` (`svelte`, `@svelters/*`) resolve to
  the host's bundle (one runtime version, smaller packages); everything else goes through LD1's `ComposedProvider`
  with `ExtensionPolicy::deny_all()` (the isolate default).
- **Bytecode in an extension bundle is ignored** unless the embedder marks the bundle trusted (first-party):
  QuickJS bytecode is trusted input only (QUICKJS-RUNTIME §3.5, ARCH-SECURITY §5 Q2 row). The host compiles the
  `js` and caches its own bytecode, keyed by content hash.
- **Dev:** `--dev <extension dir>` mounts the directory as `Dir` for `ext.<id>` (watched, §3); nothing else changes.

## 5. Milestones

| Id | Content | Size (lane-days) | Depends | Needed by |
|---|---|---|---|---|
| **AS1** | `assets` module (`AssetRef`, `Assets`, `Dir`/`Static` sources, logical paths, `register`); ref and value modules, query suffixes, `with { type: 'bytes' }`, `svelters:assets` (`asset`, `text`, `bytes`); `assets.d.ts`; `vite.js` plugin parity and `dist/assets`; egui `install` (`AssetBytesLoader`, `data:`, `SvgLoader` on resvg 0.45.1 with tint), `image.source` refs, `icon source`/`tint`, `Embed::assets`, `From<&AssetRef> for ImageSource`; `AssetRef` as RA1 prop type and in `Json<T>`; reload partition, `ReloadReport.assets`, `forget_image` + relayout. Tests: kittest an SVG icon from an import, edit the file, assert repaint with no JS evaluated (H1 stats unchanged) in embed mode; `?raw` edit goes through H1; RA1 prop round trip; Vite fixture gives the same string | 4 | Q1 (merged), H1/H3 merged (reload.rs is HMR's file family), WG1 + WG2b merged (`icon`, `images.rs`) | cadatomic's extension sidebar icons (from an unpacked dir); not SC1 |
| **AS2** | Fonts (`register_font`, JS `registerFont`/`registerIcons`, reload re-adds); CSS `url()` rewrite in the loader's and `vite.js`'s style steps; egui `background-image`/`background-size` | 2–3 | AS1, CS5e (merged) | extension icon fonts; themed panel backgrounds |
| **Q4** (extended) | `.svb` writer/reader, `svelters-bundle build|precompile|ls`, the `Layer` stack in `SourceProvider` (resolver reads through it), logical module ids, bundle-served `js`/`bytecode` with the stamp check, `Bundle::from_static`/`open`, `AssetSource::Bundle`, rasters; runner `--app app.svb`; size and startup measured with and without sources and bytecode (Q4's original acceptance) | 5–6 (was 3–4) | AS1, Q2 (disk bytecode cache) | cadatomic release builds (AP7/LI2) |
| **AS3** | Extension bundles: `kind: extension`, mount per `ext.<id>`, `externals` to the host bundle, confinement tests (an extension ref or import cannot reach `app` files), bytecode ignored unless trusted, `Bundle::file` for manifests, `--dev <dir>` mount | 2–3 | Q4, LD1 (merged) | packaged third-party extensions |

Order: AS1 → (AS2 ∥ Q4) → AS3. AS1 can start once H1/H3 and the widget lanes are merged; until then nothing else
in this document blocks a running lane. SC1 needs none of it (cadatomic's icons are font glyphs through WG1).

## 6. Open questions for the user

**Decided (user, 2026-10-22):**
- **AS-O1: (b), an `Asset` handle object**, against the recommendation. AS1 designs it so that it can cross boundaries:
  the handle wraps the stable ref (`asset://app/src/icon.svg`). Wherever an import result reaches a native prop,
  `Json<T>`, an export, an isolate or a manifest, the runtime and codegen take its ref (a well-known symbol or
  `toJSON`), so no new ABI value type is needed. JS gets methods and fields on the handle (`.url`, `.bytes()`,
  `.text()`, `.size`, change notification in dev). TypeScript types the import as `Asset`, and Vite returns the same
  handle shape.
- **AS-O2: (a)**, as recommended.
- **AS-O3: (a)**, as recommended (logical module ids, in Q4).


| # | Question | Options | Recommendation |
|---|---|---|---|
| AS-O1 | What does `import icon from './icon.svg'` give JS? | (a) a branded string `asset://app/src/icon.svg`; (b) an `Asset` handle object with methods (`.bytes()`, `.size`) | **(a)**: no new ABI type, works in props, `Json<T>`, isolates, manifests and Vite unchanged; methods live in `svelters:assets` functions taking the string |
| AS-O2 | What does a release build ship for JS? | (a) compiled JS in the bundle, QuickJS bytecode from the on-disk cache at first launch, plus an optional `precompile` step run by the app binary itself; (b) bytecode compiled at build time by a build-dependency QuickJS, required | **(a)**: the stamp always matches the engine that wrote it, cross-compiling stays possible, and a missing or stale section only costs one cold start; measure in Q4 and make `precompile` the default for cadatomic if the cold start shows |
| AS-O3 | Module ids: keep absolute paths in dev and add logical ids only for bundles, or switch everything to logical ids (`app/src/Panel.svelte`)? | (a) logical everywhere (Q4); (b) two id spaces | **(a)**: an overlay over a release bundle must hit the same module, H3 key and identity as a dev edit; real paths stay available through `Layer::real_path` for editors and stacks |

Settled here without asking (implementation inside the user's direction): a custom `.svb` container rather than
zip (§4.1); value-stable refs with Rust-side versions for dev reload (the user's preference in the request);
extension bundles confined to their root with bytecode ignored unless trusted (ARCH-SECURITY's rule, no policy
built).

## Sources

- svelters: `crates/svelters-modules/src/{lib,bundle,compose,native}.rs`, `src/source/{mod,transform}.rs`;
  `plans/milestones/{Q1-module-loader,LD1-loader-hooks,Q3w-css-scope-wiring,CS5e-egui-cascade,RA1-widget-kinds,
  RA2-json-values}.md`; `ci/h1`/`ci/h3` `plans/milestones/{H1-component-hmr,H3-state-preservation}.md` and
  `crates/svelters-host/src/reload.rs`; `ci/wg2b` `plans/milestones/{WG1-sc1-widgets,WG2b-tier0-controls}.md`,
  `crates/svelters-egui/src/{elements/images.rs,fonts.rs}`; `ci/wg4b` `plans/milestones/WG4b-dialogs-pickers-dnd.md`;
  `plans/research/{TRACKS,QUICKJS-RUNTIME,HMR,ARCH-SECURITY,ARCH-EMBED,CADATOMIC}.md`.
- egui 0.37.0 (`load.rs` `has_extension`, `Bytes`, `BytesLoader`; `context.rs` `forget_image`, `add_font`;
  `widgets/image.rs` size hints), egui_extras 0.37.0 (`Cargo.toml` features `svg` → `resvg 0.45.1`;
  `loaders/svg_loader.rs`; `image.rs` `load_svg_bytes_with_size`), usvg 0.45.1 (`Options::style_sheet`,
  `image_href_resolver`), all from the local cargo registry.
- cadatomic `d2a5789`: `docs/research/2026-10-22-{js-api-publishing-and-live-ide (§4.2, §6.3, §6.4, J6),
  native-extensions-abi-and-engines (§5.3, §6.3, §6.6, §7.2), component-framework (§5.1),
  docking-windows-and-svelters-roots (§7)}.md`; `crates/cadatomic-app/src/theme/{mod,icons}.rs`; `ui/package.json`.
- Not verified: QuickJS-NG bytecode portability across targets (assumed not portable, hence AS-O2); the deflate
  ratios and `.svb` sizes (Q4 measures); resvg's cost per icon rasterisation at typical sizes.
