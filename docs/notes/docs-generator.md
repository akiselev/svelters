# Documentation generator

Status: implemented on branch `worktree-agent-abcbaf12e578f722e` (2026-10-17). Site: `docs/site/`. Not merged.

## Tool choice: Astro Starlight (pinned: astro 7.4.0, @astrojs/starlight 0.42.6, starlight-links-validator 0.27.0)

| Option | For | Against |
| --- | --- | --- |
| SvelteKit + adapter-static | on-brand | search, dark mode, sidebar, code copy buttons, ToC, edit links and a link checker are all hand-built; the workspace pins svelte to our vendored fork tarball, which every SvelteKit/vite-plugin-svelte upgrade would have to track |
| VitePress | good defaults | Vue; Markdown generation is fine but the generated 880 pages need a custom sidebar, and there is no built-in anchor/link validation of this strength |
| **Starlight** | Pagefind full-text search (offline, static), dark mode, Expressive Code (copy buttons, Shiki highlighting for svelte/ts/rust/cpp/css), per-page `editUrl`, asides, sidebar, component overrides, maintained link validator; no Svelte dependency, so the svelte override does not touch it | Astro content sync is slow for 880 pages (about 1.5 min); a cold build is about 3 min on 4 cores |

Pages are generated as Markdown into `src/content/docs/api/` (gitignored), so Starlight handles everything downstream. There are no interactive islands; the toolkit/version switcher is a small client script in the `ThemeSelect` override. Everything builds offline after `pnpm install`: no web fonts, no CDN, Pagefind runs locally.

Build limits: `scripts/run-limited.mjs` runs astro under `taskset -c 0-3` with `UV_THREADPOOL_SIZE=4`, and `build.concurrency` is 4.

## Architecture (owner requirement: per-toolkit frontends, one shared output)

```text
metadata/{gtk,qt}/*.json  metadata/overrides  metadata/docs/{gtk,qt}/*.json
        |  tools/docs/gtk/frontend.py     tools/docs/qt/frontend.py     (future: winui, appkit, egui)
        v        each validates against tools/docs/ir/schema.json
docs/site/.generated/ir/<toolkit>.json   (svelters-docs-ir/v0, gitignored)
        |  docs/site/scripts/lib/render.mjs  (reads ONLY the IR; no toolkit code)
        v
Markdown pages -> Starlight -> dist/ -> Pagefind -> check-links
```

Site code never reads `metadata/`, GIR or qdoc. `tools/docs/build_ir.py` runs every `tools/docs/*/frontend.py`.

### IR design (`svelters-docs-ir/v0`)

- One document per toolkit: `toolkit` (id, name, version, language, status, `doc_license`, `text_policy`, `sources` with hashes, `notices`), `entities[]`, `extension_schemas`, `coverage`.
- Entity: `id` (`<toolkit>:<Qualified.Name>`), `kind` (class, interface, enum, flags, struct, boxed), `tags` (Svelte element names; elements with the same tag in several toolkits are "portable"), `status {tier: annotated|ingested, tested, annotation_files, metadata_file, note}`, `summary`/`description` text blocks, `upstream` link, `bases`/`interfaces` (entity ids, may point at undocumented types), `lifetime {class, origin: derived|manual}`, `properties`, `events`, `methods`, `constructors`, `values`, `children {policy, slots[]}`, `examples`, `screenshot` key, `serdes {status}`, `abi_symbols`, `extensions`.
- Member records carry `origin` (+ `origins`), `annotated`, `availability {since, deprecated}`, `doc`, `native_id`. Properties: `js_name`, `type {display, ref, neutral}`, `default`, `range`, `access`, `notify`, `bindable`, `role`, `accessors`. Events: `js_event`, `handler_prop`, `handler_aliases`, `params`, `returns`, `handler_result`, `sync_required`, `phase`, `flags`.
- Text blocks: `{md, license (SPDX), origin, truncated, upstream_url}`. Cross references are links with target `xref:<entity id>#<kind>:<member>`; the output resolves them or drops the link.
- Toolkit-specific facts go in `extensions.<toolkit>`, declared by `extension_schemas.<toolkit>.fields` (key, label, format text/code/list/markdown, scope entity/property/event/method). The output renders them as a generic "<title>" panel and inline member details. GTK uses it for GType, CSS nodes, accessibility, shortcuts, builder UI and GParamFlags; Qt for header, module, QSS support, ownership facts, meta-method signatures, overload collapsing.
- Future toolkits (WinUI: WinRT metadata `.winmd` types and dependency properties; AppKit/UIKit: ObjC runtime classes, KVO keys, target/action; egui: Rust builders and `Response` events) fit the same shapes: entities with properties/events/methods plus extensions. Nothing in the IR assumes GObject or Qt (for example `notify` is just a string, `lifetime.class` follows the ABI v2 vocabulary).

Tests: `tools/docs/test_ir.py` (each frontend's output validates against the schema, determinism, Qt text-policy limits, docs-pass determinism and coverage, gi-docgen xref conversion) and `docs/site/test/` (golden test of `tools/docs/ir/fixtures/mini.ir.json` through the output generator, smoke test of the counter elements for both toolkits, and of `dist/` when built).

## Documentation-text pass (`metadata/docs/{gtk,qt}/<Kind>.json`, schema `svelters-docs/v0`)

Separate from `ingest.py` so it never conflicts with the P5/serdes lanes. Keyed to the same file names and member names as the element-kind metadata (Qt methods keyed by their qdoc signature).

- `tools/metadata/gtk/ingest_docs.py`: GIR `<doc>`, `<doc-deprecated>`, `version`, `deprecated-version`, source position. gi-docgen links (`[class@Gtk.Button]`, `[method@...]`, `[property@Gtk.Button:label]`, `[signal@Gtk.Button::clicked]`, `[ctor@]`, `[vfunc@]`, enum members) become `[`Button`](xref:gtk:Gtk.Button)` etc.; `<picture>`/`<img>` are dropped; gtk-doc leftovers (`%TRUE`, `@param`, `#GtkFoo`, `|[ ]|`) are converted. 443 kinds, 3.8 MB. Rerun is byte-identical (about 50 s because the GIR is parsed with ElementTree XPath).
- `tools/metadata/qt/ingest_docs.py`: qdoc `.index` (class and property briefs, member anchors, signatures), and the HTML under `/usr/share/doc/qt6/{qtcore,qtgui,qtwidgets}` (class brief sentence, member first sentences, QSS sub-controls and pseudo-states from `stylesheet-reference.html`). `--mode links|brief|full`; committed data is `brief` (1.8 MB). `full` also writes converted Markdown descriptions (the converter handles p, code, pre, lists, links) and was checked on QPushButton.

## Licensing decision

| Text | Licence | Decision |
| --- | --- | --- |
| GTK docs (GIR comments) | LGPL-2.1-or-later | redistribute in full with attribution, licence notice and a statement that links/markup were converted (modification notice) |
| Qt docs | GFDL-1.3 | default `brief`: index briefs and first sentences, always with attribution, licence link and deep links to doc.qt.io. Switch: `tools/docs/qt/config.json` `text_policy` = `links` / `brief` / `full` (or `--text-policy`). `full` requires `metadata/docs/qt` ingested with `--mode full`, plus publishing the GFDL text and honouring modification-history rules, which the site does not do yet |
| Svelte docs | MIT | not copied; guides are written from our own notes |

Qt doc snippets are BSD-3-Clause and are not used. This is an engineering decision, not legal advice. Public page: `/about/licensing/`.

## Results

- IR: GTK 443 entities (101 elements, 8 annotated), Qt 416 (192 classes + 224 enums; 8 elements). 
- Pages built: 881 (GTK 102 + 343 type pages, Qt 9 + 409, 13 guides/concept pages, home, support matrix, coverage, indexes).
- Screenshots (generator by `tools/docs/screenshots/`, per-toolkit `gtk.py`/`qt.py`, PyGObject/PySide6 under `xvfb-run`, cached by content hash into `docs/site/.generated/screenshots`): 91 GTK and 8 Qt PNGs, 556 KB; 10 GTK kinds fail (popover kinds, emojichooser, dragicon, glarea, video, mediacontrols, two font choosers that never become ready, a flowboxchild segfault), recorded in the manifest.
- The link checker (`scripts/check-links.mjs`) verifies every internal href/src and fragment in `dist/`; `starlight-links-validator` also runs during the build.

## Known limits and open issues

- "Tested" is derived: a kind is tested when an override file exists (P3/P4 generate a verification test per annotated fact). The metadata has no per-fact test result. A request below would make it exact.
- `lifetime` is derived by heuristic (toplevel for Window/QMainWindow/QDialog descendants, otherwise owned) and marked `derived`; the proposed per-kind `lifetime` block from `ownership-and-lifetimes.md` section 2.3 is not in the metadata yet.
- Serdes support is `unknown` everywhere until the serdes lane adds fields.
- Qt property defaults and ranges are not in the metadata (only GTK has typed defaults), so Qt pages show a dash.
- GTK vfuncs are documented in the docs pass but the element-kind metadata has no vfunc records, so pages do not list them.
- `packages/renderer/types/gtk.d.ts` types handlers as `(e: SvlEvent) => void` while the renderer calls handlers with the raw signal args; the guides describe the renderer behaviour.
- Examples are generated from metadata and labelled illustrative; they are not compiled against the `.d.ts`.
- Cold site build is about 3 minutes. No CI workflow was added (outside this lane's directories); `pnpm docs:build` plus `pnpm docs:test` is the CI command set.
- The workspace lockfile changed (new packages for the docs workspace).

## Requests to other lanes

1. Metadata: a per-fact `tested: true` marker or a test-result manifest emitted by the generators, so "tested" is not inferred.
2. Metadata: the per-kind `lifetime` block (ownership note section 2.3), Qt property defaults/ranges, GTK vfunc records.
3. Serdes lane: a per-type `serdes: {status, ...}` field in `metadata/*/*.json`; the frontends will map it into `entities[].serdes` and the coverage block.
4. Annotation lanes: keep overrides under `metadata/overrides/<toolkit>/<Kind>.yaml` (the Edit link relies on that path).
5. Infra: a CI job running `pnpm docs:build && pnpm docs:test` (needs Node 22, pnpm 10, Python with jsonschema and PyYAML; screenshots additionally xvfb, PyGObject, PySide6).
