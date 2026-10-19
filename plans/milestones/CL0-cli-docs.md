# CL0: `svelters docs`, the offline docs probe (CLI phase C0)

Lane CL0 on master bff6a637. Design: `plans/research/CLI.md` §4 and §6 (C0). Evidence: `docs/notes/cli-research.md`.
Code: `crates/svelters-cli` (binary `svelters`), `docs/site/scripts/site-addresses.mjs`.

## Scope

In: `svelters docs [<toolkit>] <element>[.<member>]` (address resolver with inheritance, `-k` search, `-t`, `svelters.toml`
toolkit, did-you-mean), markdown output, `--brief`, `--json` (schema below), `--all`, `--section`; the docs pack and its
builder `svelters docs-pack`; golden, resolver and pack/site consistency tests; CI in `js.yml`.

Out: create/dev/build/run/doctor (C1), MCP (decision 7), publishing (C3), the style section (C1.5, see below), old-tag aliases.

## Decisions

1. **The CLI is Rust** (user, 2026-10-19), recorded in `plans/research/CLI.md` §5, the Summary and Decisions 9. Reason: the
   runner, the compiler and the future module loader are Rust, and `plans/research/QUICKJS-RUNTIME.md` §3.6 removed the "needs
   Node for Vite" constraint. npm stays the package manager. Until the dev module loader lands, C1's `dev`/`build` spawn Vite or
   `svelters-compile`; nothing in C0 assumes either (`docs` touches no bundler, and `lib.rs::run` dispatches subcommands, so C1
   adds modules next to `docs.rs`).
2. **Qt IR tag gap: verified, none.** Re-measured after N1 with a fresh `build_ir.py`: Qt 54 tagged entities / 55 tags = the 55
   distinct override tags (QWidget carries `box-layout` and `grid-layout`); GTK 72 = 72. The "8 of 57" in the research note was a
   stale IR. Nothing to fix; `docs/notes/cli-research.md` §1 updated.
3. **The pack builder is Rust, inside the CLI** (`svelters docs-pack`, `pnpm docs:pack`), not Python next to `build_ir.py`. The
   projection structs (`src/pack.rs`) deserialize from the IR (unknown fields ignored, so the IR can grow) and serialize into the
   pack, so the writer and reader share one definition and cannot drift; tests build packs in-process from fixture IR; no
   second Python pass in `docs:build`. The site-side routes and anchors are recomputed in Rust (`slugify`, `memberAnchor` order)
   and the consistency test pins them to `render.mjs`.
4. **Pack format: a directory plus per-entity raw-deflate JSON blobs**, not one gzip JSON. A lookup inflates the header and only
   the entity and its ancestors. Measured (release build, this machine, 2026-10-19):

   | | GTK | Qt |
   | --- | --- | --- |
   | IR (`docs/site/.generated/ir`) | 7.31 MB | 8.16 MB |
   | projected JSON (uncompressed) | 4.23 MB | 4.22 MB |
   | one deflate stream (level 9, measured with Python zlib) | 0.65 MB | 0.38 MB |
   | **`.svpack` (per-entity deflate, level 9)** | **0.92 MB** | **0.59 MB** |
   | build time (`docs-pack`, release) | 277 ms | 253 ms |

   | `svelters` call (release, both packs, wall per call, average of 10) | ms |
   | --- | --- |
   | `docs` (toolkits listing: two headers) | 11 |
   | `docs gtk button` / `docs gtk button.visible` | 15 / 14 |
   | `docs qt main-window --json` / `docs gtk widget --all` | 17 / 18 |
   | `docs gtk butonn` (miss: fuzzy over the directory, plus search) | 18 |
   | `docs -k dock` (inflates all 871 entities) | 173 |

   The per-entity layout costs ~0.5 MB against one stream and buys ~10× faster single lookups (a whole-stream inflate + parse of
   8.4 MB JSON is what `-k` pays). Release binary: 1.6 MB without packs, 3.1 MB with both embedded; links only libc and libgcc_s.
5. **How the binary finds packs** (`src/locate.rs`), first hit per toolkit: `--pack`/`--ir`/`SVELTERS_DOCS_PACK` (exclusive);
   inside the repo `docs/site/.generated/pack/<tk>.svpack`, rebuilt from the IR when missing or older (≈0.5 s, so agents in the
   repo need no extra step after `pnpm docs:ir`); packs compiled in with `SVELTERS_DOCS_EMBED_DIR=<dir>` at build time
   (`build.rs`, `include_bytes!`; checked by hand: an embedded build answers from `/`); `<exe dir>`, `<exe dir>/docs`,
   `<exe dir>/../share/svelters/docs`. Offline always, no GTK/Qt linkage. The project-locked `node_modules/@svelters/docs-<tk>`
   lookup of CLI.md §4.3 waits for C3, when the package exists.
6. **Style properties: not in C0.** The IR does not carry the ST1 style table yet (the site's styling pages read
   `packages/style/src/props.generated.js` directly, `docs/site/scripts/lib/styling.mjs`), so per scope the pack has no style
   section. C1.5 should first move the table into the IR (CLI.md §4.3), then the pack gets it for free.
7. **Light dependencies:** `serde` (derive), `serde_json`, `miniz_oxide` (pure Rust, one dep). No clap: one subcommand with ten
   flags is ~60 lines of hand parsing. Recommendation for C1: adopt `clap` (derive) only when `create/dev/build/run/doctor` land
   with nested options and completions; until then keep the hand parser. No TOML crate: `svelters.toml` is read with a line parser
   for `[app] toolkit` only (C1 owns the full config). Release build of the crate with its deps from scratch: 28 s on this machine (sccache may have helped).
   Root `Cargo.toml` gets `[profile.dev.package.miniz_oxide] opt-level = 2` (debug deflate is ~10× slower).
8. **Resolver rules** (beyond CLI.md §4.1):
   - entity keys in priority order: tag, entity id, qualified name (`Gtk.Button`, `QMainWindow::DockOption`), native name
     (`GtkButton`, `QMainWindow`), short name; `.` and `::` split entity from member, longest entity first;
   - member kinds `prop: event: method: ctor: value: slot: attr:`; events also match `js_event`, the handler attribute
     (`onclicked`) and aliases; a second, looser pass ignores `-`/`_`;
   - inheritance follows the site: own members, then bases nearest first, then interfaces; first owner per kind wins; slots come
     from the nearest entity with slots (`<application-window>` gets `<window>`'s); undocumented bases (GObject.*, QObject) are
     skipped;
   - not ambiguous: a method that is the accessor of a matching property (Qt `text()` vs `text`); a property named like a
     matching slot (GTK `child`, `titlebar`: the slot wins; DM1 marks those properties dangerous);
   - genuinely ambiguous, exit 2: e.g. `gtk button.activate` (event on Button, method on Widget), `qt main-window.close` (the
     synthetic close event and `QWidget::close()`);
   - outside a project, a name in several toolkits lists one candidate per toolkit (exit 2); a name in one prints a note
     ("found only in gtk"); inside a project only its toolkit is searched, and naming another adds "this app targets qt".
9. **Doc text in the terminal:** xref links become their text (site links are only meaningful on the site), images are dropped,
   the site's HTML allow-list becomes markdown, other tags stay literal like on the site. Notices (GFDL attribution for Qt) are
   printed under any doc text and returned as `notices` in JSON.

## `--json`: schema `svelters-docs/1`

Every result is one JSON document: `{"schema": "svelters-docs/1", "kind": <kind>, …, "notes": [string]}`. `notes` carries what
the markdown output prints to stderr (toolkit notes, warnings). Stability: fields are only added; absent optionals are omitted,
never `null`; arrays are always present. Exit status as in markdown: 0 found, 1 `not_found`, 2 `ambiguous`, 3 usage, 4 no data.

| `kind` | Command | Fields |
| --- | --- | --- |
| `toolkits` | `docs` | `project?: {toolkit, config}`, `toolkits: [Toolkit]` |
| `index` | `docs <tk> [--all]` | `toolkit: Toolkit`, `elements: [IndexRow]`, `types: [IndexRow]` (only with `--all`), `types_total` |
| `entity` | `docs [<tk>] <entity>` | `toolkit` (id), `data_version`, `entity: Entity`, `children_policy?`, `default_slot?`, `properties`, `events`, `slots`, `methods`, `constructors`, `values`: `[Member]` (own; inherited too with `--all`), `dangerous: [DangerRef]` (own + inherited, always complete), `inherited: [{from, name, properties, events, methods}]` (counts), `all: bool`, `examples: [{title, lang, code, generated?, tested?}]`, `notices` |
| `member` | `docs [<tk>] <entity>.<member>` | `toolkit`, `data_version`, `entity: {id, address, native, tags, site}`, `member: Member`, `notices` |
| `search` | `docs -k <q>` | `query`, `toolkits`, `results: [Candidate]`, `truncated` |
| `ambiguous` | exit 2 | `query`, `candidates: [Candidate]` |
| `not_found` | exit 1 | `query`, `toolkits`, `suggestions: [Candidate]` |

- **Toolkit**: `id, name, version, data_version, source, elements, entities, doc_license {spdx, name, url?, redistribution?},
  text_policy?, notices, extension_labels {key: label}`. `data_version` = `<toolkit version>+<FNV-1a 64 of the IR bytes>`.
- **IndexRow**: `address, id, native, tags, kind, tier, tested, brief, site`.
- **Entity**: `id, address, kind (class|interface|enum|flags|struct|boxed), name, native, qualified?, tags, tier
  (annotated|ingested), tested, verified_facts? [verified, total], abstract, summary?, description?, site, upstream?, bases,
  interfaces, lifetime?, since?, extensions {<tk>: {...}}, see_also`. `--brief` drops `description`, `extensions`, examples and
  shortens `summary` to its first paragraph.
- **Member** (all kinds): `kind (property|event|method|constructor|value|slot|attr), name, address, declared_in, site,
  annotated, verified, since?, deprecated?, doc?, dangerous? {severity, reason, use_instead?, reachable, source}`; `--brief`
  drops `doc`. Plus per kind:
  - property: `native, type {display, native?, ref?, nullable?}, access (rw|r|w|construct-only), default?, range?, notify?,
    bindable, accessors?`;
  - event: `native, attr?, aliases, params [{name, type}], returns?, sync, result?, phase?, flags`;
  - method / constructor: `signatures` (one per overload), `params, returns?, static, virtual, unsupported?`;
  - value: `nick?, value`; slot: `default, cardinality, accepts, attrs, order?, ops, ownership, on_remove?`;
  - attr: `attribute` (`parent:<name>`), `slot`.
- **DangerRef**: `kind, name, address, declared_in` + the `dangerous` fields flattened.
- **Candidate**: `address, toolkit, kind (element|type|<member kind>), declared_in?` (inherited members only), `brief, site`.

`address` is always a valid `svelters docs` argument line (`gtk button.prop:can-shrink`, `qt QMainWindow::DockOption.value:AnimatedDocks`).
`site` is a path on the docs site (`/api/gtk/button/#property-can-shrink`). Examples: `crates/svelters-cli/tests/golden/*_json.txt`.

## Tests

- `src/*` unit tests: slugify/anchor rules, FNV, text clean-up, address splitting, member prefixes, fuzzy, `svelters.toml`.
- `tests/golden.rs` + `tests/golden/*.txt`: 40 goldens (md, `--brief`, `--json`) on `tests/fixtures/ir/{gtk,qt}.json`, a
  checked-in 262 KB subset of the real IR made by `tests/fixtures/make_fixture.py` (so metadata edits by other lanes do not churn
  goldens): GTK button/application-window/window/grid/enum, Qt main-window/push-button/enum; inherited members
  (`button.visible`, `application-window.titlebar`), DM1 danger marks (`window.set_titlebar` reachable, `QMainWindow::setCentralWidget`
  not), overloads, slot/attr, ambiguity, misses, search, toolkit notes; plus usage/no-data exit codes, `svelters.toml`, `docs-pack`
  write/check/`--pack` and a damaged pack. `UPDATE_GOLDEN=1` rewrites.
- `tests/site_consistency.rs`: renders the site from the real IR (`site-addresses.mjs` over `render.mjs`) and checks every page
  route, tag, own member anchor and inherited link against the pack built from the same IR (871 pages, 15,240 own anchors,
  54,619 inherited links on 2026-10-19); also pack-build determinism. Checked to fail: emitting property anchors from `js_name`
  gives 1,353 mismatches.

## Open items

- Style section (C1.5): move the ST1 style table into the IR first.
- Old (pre-N1) tag aliases with "renamed to" lines (CLI.md §4.1): needs the rename map as data, not just
  `docs/notes/n1-rename-map.md`.
- Data findings, not CL0's to fix: 273 Qt properties (all unannotated) still have camelCase `js_name`s (`dockOptions`,
  `iconSize`), which the CLI prints as is; the site routes QWidget to `/api/qt/box-layout/` because that is its first tag; GTK
  `notify::<prop>` events appear as events with handler `onnotify::<prop>`.
- `-k` inflates every entity (173 ms); a member-name index in the header would make it ~20 ms if agents search a lot.
- `--section` is per card only; no `--section` on the toolkit index.

## Completion record

Branch `ci/cl0` (worktree branch `worktree-agent-afc8aa27c879ff599`), based on master bff6a637. Not merged.

Commits: a123618d (crate, pack, resolver, output, tests, CLI.md Rust decision), f04ac818 (CI, `docs:pack`, this file,
CLI.md §4.3, cli-research note), plus this record.

Gates, observed locally on 2026-10-19 after the last code edit:
- `tools/dev/cargo-slot cargo nextest run -p svelters-cli`: 60 tests run, 60 passed, 0 skipped (17 unit, 42 golden/CLI, 1 pack/site
  consistency: 871 pages, 15,240 own anchors, 54,619 inherited links).
- `tools/dev/cargo-slot cargo clippy -p svelters-cli --all-targets -- -D warnings`: clean.
- `tools/dev/cargo-slot cargo check --workspace --all-targets`: exit 0 (warnings only in other crates).
- `corepack pnpm docs:test`: exit 0; Python 11 tests OK (1 skipped), node 17 tests, 16 pass, 0 fail, 1 skipped (no `dist/`). Locally
  the nested `pnpm` call needs a `pnpm` shim on PATH (only `corepack pnpm` is installed here).
- `pnpm test:meta`: not run; CL0 touches neither `tools/metadata` nor `tools/docs`.
- Embedded build (`SVELTERS_DOCS_EMBED_DIR`): checked by hand from `/`; not in CI.

CI on `ci/cl0` @ f04ac818, all success: js 39294201831 (runs the svelters-cli step: clippy, 60/60 nextest, docs-pack with the
same pack sizes as locally, smoke calls), linux-gtk 39294201703, linux-qt 39294201749, macos-appkit 39294201602,
windows-winui 39294201722.

Shared files touched: root `Cargo.toml` (one member, one `profile.dev.package` entry), `Cargo.lock`, root `package.json`
(`docs:pack`), `.github/workflows/js.yml` (one step), `plans/research/CLI.md`, `docs/notes/cli-research.md`. `STATUS.md` untouched.

Open items: see above.
