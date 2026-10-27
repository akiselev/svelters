# POL1-GALLERY: the gallery tool, so the polish lanes see true images (batch polish-b1, lane gallery)

Branch `ci/pb1-gallery`. Brief: `plans/cloud/polish-b1/BRIEF.md` and `LANE-gallery.md`. Base: `master` `9d97e7d`
(merged `origin/master` `2a35968` at the end, no conflicts). Status: **items 1 to 7 done.** CI: see the end.

## Scope

`crates/skelter-gallery*` (CLI, renderer, the app's fonts and assets), `gallery/` (the `window` story; the icon
story's note), `docs/skelter/gallery.md`, and the small changes the requests name: `svelters-style` + its two
`MatchedRuleInfo` builders (skelter-core `style/inspect.rs`, svelters-egui `style.rs`) for R1, `svelters-host`
(`quickjs.rs`) for R3, `svelters-modules` (`lib.rs`) for R4, `skelter-widgets/Cargo.toml` + `Cargo.lock` for Q1.
One change outside the brief's list: **skelter-text** `FontStore::symbol_chain`/`set_symbol_chain` (item 1, below).
No `base.css` rule and no kind's look was touched.

## What landed

| # | Item | Commit | What |
|---|---|---|---|
| 1 | Fonts | `cd478f3` | `crates/skelter-gallery/fonts/`: OFL subsets (≈1.5 MB) of the theme's own families, **Inter** 300/400/600/700 and **JetBrains Mono** (`--sv-font-family`, `--sv-font-mono`), plus Noto fallbacks: Arabic, a CJK UI-word sample (SC + KR, `cjk-sample.txt`), Symbols, Symbols 2, Math; README with sources and licences, reproducible `subset.py`. `skelter_gallery::fonts::{store, FontSet}`; `--system-fonts` adds the OS's fonts behind them. The app uses the same set (system fonts on) |
| 1 | skelter-text | `cd478f3` | `FontStore::symbol_chain()` / `set_symbol_chain()`: the per-OS symbol chain (`Segoe UI Symbol`…, `Apple Symbols`…, `Noto Sans Symbols 2`…) becomes the default and is settable, so the gallery's symbols draw the same on Windows, macOS and Linux. Default behaviour unchanged (its 41 tests pass) |
| 2 | Icons, assets | `cd478f3` | `skelter_gallery::icons`: a demo set of 20 host-key line icons (`file.*`, `edit.*`, `search`, `settings`, `close`, `add`, `remove`, `check`, `run.*`, `refresh`, `folder`, `warning`, `info`; drawn for the gallery, `currentColor`); a story directory's `.svg`/`.png` registered as image sources under the `asset://app/…` refs their imports evaluate to (GAL2 requests 3 and 4). The app does both too |
| 3 | Header escaping | `bdf3264` | `model::escape_label`: an arg value's newline, tab and control characters print as escapes in labels and headers |
| 4 | `--diff <before.png>` | `bdf3264` | on `shot` and one-story `sheet`: writes `<out>.diff.png` (before \| after \| changes: after faded to grey, changed pixels magenta, the region framed red) and reports changed pixels and bounds (stderr; `"diff"` in `--json`). `skelter_gallery::diff` |
| 5 | `Renderer::reload(paths)` | `a75d4dc` | a `story.toml` re-reads the stories; an image is registered again (no restart); a variant that failed to load is retried; a change in a loaded variant's module graph replaces the JS runtime (a fresh `Embed` on the same backend, document, window and GPU device). Returns `Reload { stories, assets, variants, restarted }`. GAL1 Q1 |
| 6 | `window` story | `258ce32` | a variant whose root creates a `<window>` gets that window as the headless window's root for its cell, laid over the theme's `bg`; `inspect` searches it; the gallery's window is restored for the next cell. The harness now clears to transparent (the slot paints every ordinary cell). Story `layout/window` (`content`, `app`): `list` shows no kind without a story |
| 7 | R1 | `9e8a37f` | `MatchedRuleInfo::layer` (`Rule::layer_name`: `base.buttons`, `<anonymous>`); inspect prints it (`sv.base` for base.css's kind rules) |
| 7 | R4 | `9e8a37f` | svelters-modules' `canonicalize` is public; `skelter_gallery::canonicalize` re-exports it (the copy is gone) |
| 7 | R3 | `5d837dd` | the full module-load message: QuickJS formats a loader error into a 256-byte buffer, so a long path cut it (`…[js_parse_error]: Un`). The loader and resolver keep the last failure's message; the engine's load error gets it back when it names that module |
| 7 | Q1 | `6176723` | skelter-widgets' `quire-text` at `ed0665b` (was `35b5063a`): one quire-text in the lockfile |

## Findings (rows of GAL2's "Polish findings" and GAL1/GAL3 notes in this lane's scope)

| Finding | Cause | Fix (file) | Before/after looked at | Snapshot tests moved |
|---|---|---|---|---|
| Arabic draws as tofu (`label/rtl`, every `rtl` variant) | the hermetic test fonts have no Arabic | bundled Noto Sans Arabic (`skelter-gallery/fonts`, `src/fonts.rs`) | yes, 4 themes, zoom 1 and 2 | none |
| Symbols ★ ⚙ ◀ ✎ ✂ ⌫ tofu (`icon/glyph`, `icon/button`, `icon/toolbar`) | no symbol font; skelter-text's symbol chain names OS fonts | bundled Symbols/Symbols 2/Math + the gallery's chain (`fonts.rs`, skelter-text `set_symbol_chain`) | yes, 4 themes, zoom 1 and 2 | none |
| No CJK coverage (GAL1 Q3) | test fonts | a CJK UI-word sample (Noto Sans SC, KR); outside the sample: boxes, or `--system-fonts` | yes (test shapes zh/ja/ko with no `.notdef`) | none |
| `label/weight` 300 draws as 400 | no light face | Inter 300/400/600/700 bundled: the four weights differ | yes | none |
| `rich-text` `<code>` not monospace (part) | no mono font in the set | JetBrains Mono bundled (the token's first family). The field fill is the text lane's | yes | none |
| Cells drew Noto Sans, not the theme's Inter | `FontConfig::testing()` | Inter is the default family | yes | none |
| `icon/registry`, `button/icon`: key text / empty box | no icons registered (GAL2 req. 4) | demo icon set (`src/icons.rs`) | yes, 4 themes, zoom 1 and 2 | none |
| `image/*`, `icon/source`: placeholder box | `asset://` refs not resolved (GAL2 req. 3) | story images registered under their refs (`icons::register_assets`) | yes | none |
| `multiline/value` header overlaps | the arg's newline printed raw | `model::escape_label` | yes | none |
| `window`: no story | a toplevel can't sit in a slot | window stories (`render.rs` `present`/`unpresent`) + `layout/window` | yes, 4 themes | none |
| **New:** `<window padding gap>` have no effect | the window's `padding`/`gap` props never reach its style (`inspect window/content window` shows neither) | **not fixed** (svelters-skelter's window kind mapping, not this lane's files); the story shows it (`padding = [12, 0]` rows look the same) | yes | — |

No pixel snapshot test moved: no existing test renders through the gallery's fonts (the gallery has no goldens; the
app's tests keep `FontConfig::testing()`).

Before/after images (container paths, not committed):
- `target/gallery/before/label-rtl.png` → `target/gallery/after/label-rtl.png`: Arabic boxes → Arabic text.
- `target/gallery/before/icon-glyph.png` → `target/gallery/after/icon.png`: a tofu box → ★; `file.save` text → the
  save icon; the imported gear SVG; ◀ ▶ ✎ ✂ ⌫ in the toolbar.
- `target/gallery/before/multiline.png` → `target/gallery/after/multiline.png`: overlapping header → one line.
- `target/gallery/after/window.png`: the window story in four themes; `target/gallery/d/after.diff.png`: `--diff` of
  a `border-radius: 0` edit (the four corners in magenta).

## Tests

`SKELTER_GPU_TESTS=1 WGPU_BACKEND=vulkan cargo test -p skelter-gallery` (last run after the final code change and the
`origin/master` merge: all pass; `skelter-gallery check`: 32 stories, 137 variants, 0 problems).

| Test | Count | What |
|---|---|---|
| `skelter-gallery` `tests/media.rs` | 5 (3 GPU) | fonts cover Arabic/Hebrew/zh/ja/ko/symbols with no `.notdef`, Inter and JetBrains Mono found by name, the symbol chain; demo icons resolve and a story's SVG registers under its ref; an imported SVG and a registry icon draw; `reload` (image, variant restart, broken then fixed, unrelated path); a window story draws its window and the next cell is ordinary |
| `skelter-gallery` `tests/diff.rs` | 2 | counts, bounds, panels, size change; PNG round trip |
| `skelter-gallery` `tests/model.rs` | +1 | labels stay on one line |
| `skelter-gallery` `tests/render.rs` | 1 extended | `layer` is null for a component rule, `sv.base` for base.css's `button` |
| `svelters-style` `layer_names_for_inspectors` | 1 | unlayered, nested, anonymous |
| `svelters-modules` `canonicalize_is_absolute_and_never_verbatim` | 1 | |
| `svelters-host` `tests/load_errors.rs` | 2 | a load and a resolve error keep their whole message under a long path |
| run, unchanged | | skelter-text (41), skelter-widgets `text::` (45) and `text_pins`, svelters-host `modules`, `hmr`, `hooks`, `reload` (`reload` needs `cargo build -p svelters-compiler` first) |

Not run locally: the full workspace tests, clippy beyond the crates above (advisory in CI and already red in
unrelated crates), the other workflows (GitHub CI is the gate).

## Questions and requests (each with a recommendation)

- **To the coordinator (svelters-skelter, a later lane): the window's `padding` and `gap`.** The `window` surface lists
  them, but they don't reach the node's style. Recommendation: map them as `box` does; the `window/content` story then
  shows four distinct rows.
- **Image sources are the process's, keyed by asset ref.** Two gallery roots with the same relative path
  (`asset://app/stories/…/x.svg`) share one image source; the last registered wins. Harmless for one gallery;
  recommendation: leave it until a host mounts two roots with colliding paths (then key by root).
- **The interactive app does not show window stories** (its cells are slots in one window). Recommendation: leave it;
  the CLI is the reference for them.
- **`--system-fonts` is not deterministic** by design; the bundled CJK set is a sample. Recommendation: add words to
  `fonts/cjk-sample.txt` when a story needs them, rather than shipping the 10–18 MB fonts.

## Handoff

Nothing open in the lane. Other lanes merge `origin/ci/pb1-gallery` (a merge, never a rebase) for fonts, icons,
`--diff` and `reload`. `crates/skelter-gallery/fonts/subset.py` rebuilds the font set.

## CI

(filled in below)
