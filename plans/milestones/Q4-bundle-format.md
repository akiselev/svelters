# Q4: the release bundle format (`.svb`)

Design: `plans/research/ASSETS.md` §4 (the `.svb` container, which replaces `app.qbc`) and AS-O2 (a),
`plans/research/QUICKJS-RUNTIME.md` §3/§6 row Q4, `plans/research/TRACKS.md` §2.1 LOADER row Q4. Builds on Q1
(`ModuleProvider`), Q2 (`svelters-host/src/bytecode.rs`), AS1 (`Assets`) and LD1 (`ComposedProvider`). Lane: Q4,
branch `ci/q4`.

This lane is the core of ASSETS.md's "Q4 (extended)" row: the format, its reader and writer, the builder, the
provider, `precompile`, and a host load path. The rest of that row is listed under "Deferred".

## What landed

### The container (`svelters_modules::svb`)

```
"SVB1" | u32 LE index_len | index (UTF-8 JSON) | zero padding to 16 | blob area (each blob 16-byte aligned)
```

```jsonc
{
  "format": 1, "kind": "app" | "extension", "root": "app", "version": "<package.json version>",
  "build": "<hash of the index>",
  "stamp": { "compiler": "<svelters version>", "runtime": "<@svelters/native-runtime version>",
             "engine": "quickjs-ng", "engine_build": "<svelters_host::bytecode::stamp()>" | null },
  "entry": "svelters:entry",
  "files":   { "<path in root>": { "blob": 3, "hash": "<fnv1a64 hex>", "codec": "none",
                                   "role": "source" | "manifest" | "asset", "mime": "image/png" } },
  "modules": { "app/src/App.svelte": { "js": 0, "map": 1, "bytecode": 2, "source": "src/App.svelte",
                                       "source_hash": "<hex>", "imports": { "./state.svelte.js": "app/src/state.svelte.js" } } },
  "sections": { "<name>": { "version": 1, "required": false, "blob": 7 } },
  "meta": { },                                        // embedder data, opaque
  "blobs": [ [offset, len], … ]                       // offsets relative to the blob area
}
```

- `Svb::from_static(include_bytes!(…))` serves blobs zero-copy (assets come back as `AssetBytes::Static`).
  `Svb::from_bytes(Vec<u8>)` and `Svb::open(path)` read the whole file. The reader checks the magic, the
  index and every blob's bounds, and that blob references and the entry are valid. It refuses another
  `format` and any codec other than `none`.
- `SvbWriter` deduplicates blobs by content (FNV-1a, then a byte compare, so a hash collision never merges
  different bytes). It drops blobs nothing refers to and gives deterministic output: the same input gives
  the same bytes and the same `build`.
- **The section table, for pre-parsed CSS.** `SvbWriter::section(name, version, required, bytes)` and
  `Svb::section(name) -> Option<(version, &[u8])>`. A reader skips sections it does not know. A section
  marked `required` that is not in `KNOWN_SECTIONS` makes the reader refuse the bundle. OW-CSS's parsed
  stylesheets can be added as an optional section (for example `css.parsed`, version 1) without a format
  change. Its version moves with the parsed-sheet encoding, and an unknown version is a reason to re-parse.
  Nothing is stored there yet.
- `Svb::with_bytecode(map, engine_build)` rewrites a bundle with bytecode, for `precompile`.

### The builder (`svelters_modules::svb::build`, feature `source`)

`build(SourceConfig, &BuildOptions { sources, meta }) -> Vec<u8>` is pure Rust and needs no JS engine, so it
can run from a `build.rs` before `include_bytes!`. It runs `SourceProvider` with `dev` and `hmr` off (the
production condition). It walks the module graph from the entry:

- it loads each module (compiled JS and its map);
- it collects the module's imports with the compiler's acorn port: static imports, re-exports, and `import()`
  of a string literal (a computed `import()` is a build error);
- it resolves each import with the provider and records it in the module's `imports` table;
- it leaves out host modules (`svelters:*` except `svelters:behaviors/*`), which the host serves first.

What it stores:

- **Logical module ids** (AS-O3): `app/<path in the app>`, `app/node_modules/<package>/<path>` for
  files outside the app (found through the nearest named package.json, as `Assets::register` does), with
  the import query kept (`app/src/note.txt?raw`). Generated modules keep their ids (`svelters:entry`,
  `svelters:kinds`). If two files would get the same id (two copies of a package), the build fails.
- **Files.** Each imported asset is stored under its ref path (`src/icon.png` for
  `asset://app/src/icon.png`). The source of each code module is stored when `sources` is on (decision 5:
  on by default), together with its `source_hash` either way. Every package.json the graph touches is
  stored as a manifest.

### The provider (`SvbProvider`) and the host

- `SvbProvider::new(Arc<Svb>)` mounts the bundle's files as its asset root (`AssetSource::Bundle`, new in
  `assets.rs`). `resolve` is a lookup in the base module's import table. No resolver runs, and no file
  system is touched. `load` gives the bundle's bytecode (`ModuleCode::Cached`) when the provider is
  trusted and `engine_build(stamp)` names exactly the bundle's `stamp.engine_build`. Otherwise it gives
  the JS and its map, and the host compiles that through Q2's on-disk cache: AS-O2 (a), bytecode cached
  at first launch.
- Trust: an app bundle is trusted by default. An extension bundle's bytecode is ignored unless
  `.trusted(true)` (QUICKJS-RUNTIME §3.5, ARCH-SECURITY §5). AS3 wires the policy.
- `Host::load_svb(Arc<Svb>) -> Rc<SvbProvider>` loads through `load_from`, so LD1's composition applies.
  It passes `bytecode::stamp()`.
- `svelters_host::bytecode::precompile(&Svb) -> Vec<u8>` is ASSETS.md's `precompile`. It compiles each
  module's JS in a fresh QuickJS runtime of the running binary (compile only, nothing is evaluated) and
  stamps the bundle. It never runs at build time.

### ZU3

All bundle file I/O (`Svb::open`, the builder's reads) is in svelters-modules, which the lint does not cover.
In svelters-host, `precompile` does no I/O and sits in `bytecode.rs`, which already carries the file-level
`#[allow]` with its ZU3 reason. No new `#[allow]`.

## Decided (Q4 lane, 2026-10-22)

These are format choices the docs left open. In each case the simplest option was taken.

1. **No new dependencies.** Hashes are FNV-1a 64 (`content_hash`), not BLAKE3. This is safe for dedupe
   because blobs are byte-compared, and enough for `build` ids and overlay base hashes. Switching to BLAKE3
   later means changing the `hash` strings only, not the format.
2. **Format 1 is uncompressed** (`codec: "none"`). `deflate` is reserved, and readers refuse unknown codecs
   per file. Sizes below show what compression would save.
3. **Blob table in the index** (`blobs: [[off, len]]`), referenced by number. Content addressing is the
   writer's dedupe, not the key.
4. **Section table** with per-section `version` and `required` (above). New index keys are additive, and
   `format` changes only for changes readers cannot ignore.
5. **`entry` is a module id**, not ASSETS.md's `{ kind, names }`. The builder generates the entry module
   (app mount or embed exports), so the host needs nothing more. An embed's names can go in `meta`.
6. **`Svb::open` reads the whole file.** `memmap2` comes later, behind the same type.

## Tests

- `svelters-modules` unit tests, `svb::tests`:
  - round trip;
  - 16-byte alignment, dedupe, unreferenced blobs dropped;
  - deterministic output;
  - static zero-copy;
  - sections: versioned, an unknown optional one skipped, an unknown required one refused;
  - bad magic, truncation and a future format refused;
  - `with_bytecode`.
- `svelters-modules` unit tests, `svb::provider::tests`: import-table resolution; bundle assets; bytecode
  only with the same engine stamp and trust (extension bytecode ignored).
- `svelters-modules` unit tests, `source::build::tests`: the import scan (static, re-export, literal `import()`,
  strings, comments and regexes ignored, computed `import()` refused); host modules left out.
- `svelters-host/tests/modules.rs`, `an_app_loads_from_its_release_bundle_without_its_sources`. The Q1
  counter app (JSON, an asset read through `svelters:assets`, a runes module, a dumped behaviours module) is
  built with and without sources, then its directory is deleted. The bundle then:
  - loads from `app.svb` (first launch: every module compiled into the bytecode cache);
  - loads precompiled (every module read as bundle bytecode, nothing compiled);
  - loads without sources.

## Measurements

The Q1 counter app (126 modules: the app, `@svelters/native-runtime`, Svelte's client runtime), debug build,
uncompressed:

| Bundle | Size |
|---|---|
| JS + maps + assets + manifests, no sources | 1,007,130 B |
| with sources | 1,031,482 B (+2.4%: a plain `.js` module's source is its JS, so it dedupes to the same blob) |
| precompiled (bytecode added) | 2,299,879 B (bytecode with debug info, `write_options()`, is about 1.3× the JS) |

Startup was not timed here (see Deferred).

## Deferred (the rest of ASSETS.md's "Q4 (extended)" row)

- The `Layer` stack in `SourceProvider` (overlay → dirs → bundle) and logical ids in dev. Without these,
  live compile on top of a release bundle (decision 5, LI2) does not work yet. The bundle already ships
  sources, manifests and logical ids for it.
- The `svelters-bundle build|precompile|ls|cat` tool, the runner's `--app app.svb`, and a `--precompile`
  hidden flag.
- `package.json` `"svelters": { "assets": [globs] }`; rasters (pre-rasterised SVG); `deflate`; `memmap2`.
- Startup time with and without the compiler and with and without bytecode, in a release build (Q4's
  original acceptance). The test above prints sizes only.
- `kind: "extension"` bundles: `externals`, confinement, mounting as `ext.<id>` (AS3).
- The generated entry module imports the app's files by absolute build-machine path (resolved through the
  import table, so it works). A logical-id entry comes with the layer stack.
