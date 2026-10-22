# TC-EVAL: quire-core as the text core of every svelters field

Question (user, 2026-10-22): "look at quire's core to see if we should use that for text, to give us advanced
features everywhere (but we should measure performance impact)". Context: svelters replaces egui's widgets with
its own; text fields get a shared editing core, **TC0 = `quire-text`**, a new crate in the quire repo (buffer,
selections, ChangeSet/Transaction, undo grouping, word rules, IME composition; no egui, wasm32-clean) that
macbook-cadatomic extracts. cadatomic requires quire's Document/EditorView split. This file answers the
measurement question for ARCH-WIDGETS §4.3 ("the text backend stays open until TC-EVAL reports").

Inputs: quire at cadatomic's pin `2087b4354f76c439d2d729661e8b2fa7d7de86b0` (read only), egui 0.37.0 sources,
svelters-egui `native/words.rs` (FX1). Bench: `plans/research/tc-eval/` (not a workspace member).

## Summary

- **Recommendation: (D) used lazily (C).** TC0 = `quire-text` as ARCH-WIDGETS §4 sketches it, with a
  **flat `String` backend for small texts** (copy-on-write `Arc<String>`, so snapshots stay O(1)) and ropey above
  a threshold (64 KiB) behind the same `Text` API, history and recent-changes allocated on first edit, rules
  shared instead of a per-document `EditorConfig`, and none of the editor's per-document state (tokens,
  diagnostics, folds, find, saved text). svelters creates a `TextDoc` + `TextView` only when a field is focused or
  edited; idle fields and labels are plain strings, and selectable labels use quire-text's pure word and
  grapheme functions over `&str`.
- **quire-core as it is costs about 10x egui's field in memory and 20x per keystroke on short text.** A
  `Document` + `EditorView` with a field config is 2.6 KB per idle field (1.15 KB heap + 1.46 KB inline), against
  268 B for an egui `TextEdit` field; 10,000 PropertyGrid rows hold 25 MB against 2.5 MB and take 35 ms to build
  against 1.4 ms (one dropped frame or two). A keystroke costs 8 µs median and 27 allocations against 0.4 µs and
  3. Latency is far below a frame either way: 100 typed characters cost 0.7–1.2 ms in total.
- **Two numbers favour quire.** egui's undoer stores whole-text snapshots and clones the text twice per focused
  frame, so at 10 KiB quire types 3x faster (7.5 µs against 22 µs) and holds 6–11x less history (20.5–36 KiB
  against 233 KiB after 20 undo groups). Its word-level undo groups (20 steps for 100 characters, where egui gives
  one) are the "rich undo" the user asked for.
- **Most of what makes quire-core "advanced" does not belong in plain fields.** Multi-cursor, column selection,
  auto-closing pairs, find/replace, comments, indentation and folds are code-editor features. VS Code's word
  classifier would regress FX1's native word rules (pin 1: 72/72 today). Monaco undo grouping differs from GTK's
  and Qt's (FX3, pin 4). The parts every field gains from are the transaction model (versions, change sets,
  anchors, rebasing), the undo history (with a rule set per platform), the IME model and the grapheme handling.
  Multi-cursor in a multi-line text area can be an opt-in.
- **Gaps for UI fields, none of them in quire-core today:** single-line mode (quire's own find bar uses egui's
  `TextEdit::singleline`), password/secret mode, max length, input filter, platform word rules (only the VS Code
  separator classifier: no Unicode categories, no script breaks, no subwords), GTK/Qt undo rules, history reset on
  an external set (GTK `set_text`, Qt `setText`), an undo-stack cap (the stacks are unbounded), and setting
  X11 PRIMARY from a selection (egui_quire handles only middle-click paste).

## 1. quire-core: API and features

quire-core (14.2k lines incl. tests, edition 2024, `unsafe` forbidden, no egui, no threads, wasm32-clean) is the
headless model of quire. egui_quire paints it and quire-syntax adds tree-sitter (highlights, folds, bracket pairs).

| Feature | In quire-core | Notes for a field |
|---|---|---|
| Buffer | `Text`: ropey 1.6 behind a small API, UTF-8 byte offsets, LF only in memory (CR/CRLF and BOM recorded for files), O(1) clone | ropey's leaf node is a fixed 1 KiB allocation: an empty `Text` is 1,024 B of heap |
| Versions, change sets | `Version`, `ChangeSet` (compose, invert, map, `map_pos` with `Bias`, `diff` via imara-diff), `Transaction` (changes + selection + effects + origin + group hint + time) | the right model for `bind:value`, AccessKit and remote edits |
| Document / view split | `Document` (text, version, history, recent changes, tokens, diagnostics, saved text, config, folds) and `EditorView` (selection, folds, find, scroll, focus, preedit, auto-closed pairs, session); `apply` / `apply_remote`, views `sync` through the recent-changes window | already the shape cadatomic requires; `Document` is 984 B inline and `EditorView` 472 B |
| Selections | `Selection` = `SmallVec<[SelRange; 1]>` + primary; `SelRange` anchor/head/goal column | one range stays inline |
| Multi-cursor | `InsertCursorAbove/Below`, `AddNextMatch` (Ctrl+D), `RemoveSecondaryCursors`, per-cursor paste | editor feature |
| Column selection | `ColumnSelect{Left,Right,Up,Down,PageUp,PageDown}`, `ColumnSelect` | editor feature |
| Undo | `History`, Monaco grouping (word boundary, cursor jump, pause `undo_pause_ms`, non-typing commands own group, `GroupHint`), `UndoMode::Local` or `Host` (host owns undo; closed groups reported as `UndoGroupClosed`), foreign changes rebased over | no cap on the stacks; one rule set (Monaco) |
| Word rules | `WordClassifier` (VS Code: whitespace / configured separators / regular), word motions and deletions (`WordLeft`, `WordStartLeft`, `WordEndRight`, ...), `word_range_at` (double click) | not Pango or Qt (FX1); no subword (camelCase) motion; a 10 KiB single line is copied to `Vec<char>` per motion |
| Graphemes | `unicode-segmentation` cursor per motion | over a `String` copy of the line |
| Bracket matching | not in core (quire-syntax, from tree-sitter queries); core has bracket rules for Enter indentation and auto-close pairs | auto-close must be off in fields |
| Find / replace | `FindQuery` (regex, case, whole word) on the view, `FindNext/Previous`, `ReplaceOne/All`; regex-automata + fancy-regex | editor feature; the regex stack is most of the compile cost |
| IME | `Preedit { text, active }` on the view, `EditorCommand::CompositionType` (VS Code: replace n chars before/after), egui_quire maps `egui::Event::Ime` | reusable as is; no GTK hex entry |
| Anchors | `Document::anchor(pos, bias)` / `resolve_anchor`: mapped through the recent-changes window (64 change sets by default), `Stale` beyond | fine for fields |
| Change events | `Applied` (versions, rebased changes, origin, selection changed, config changed, closed undo groups), `SettledProbe`, `RecentChanges` | the source for `Edited{version}` and AccessKit updates |
| Clipboard | `Cut`/`Paste` commands; copy is the widget's; `HostRequest` (`Save`, host `Undo`/`Redo`) | no PRIMARY ownership |
| Commands | `EditorCommand`, about 70 VS Code ids, resolved purely (`Document::command`) then applied (`execute`) | |
| Tests | `tests/monaco.rs` (VS Code port), `laws.rs` (change-set laws), `robust.rs` (proptest), `document.rs`, `folding.rs` | the port is the asset to keep |

**Dependencies (normal, 15 crates):** ropey + str_indices, smallvec, imara-diff (+ hashbrown, foldhash),
unicode-segmentation, regex-automata (full features) + regex-syntax + aho-corasick + memchr, fancy-regex
(+ bit-set, bit-vec). No tree-sitter, no egui. Not in svelters' `Cargo.lock` today: ropey, str_indices,
imara-diff, regex-automata, regex-syntax, aho-corasick, fancy-regex (bit-set/bit-vec only in other versions).

**Compile cost** (release, 5 jobs, no sccache, load average 12–14):

| Build | Wall time |
|---|---|
| quire-core and all its dependencies, clean | 51.9 s |
| quire-core alone (after touching `lib.rs`) | 14.4 s |
| `cargo check -p quire-core` (dev, clean) | 15.7 s |
| only fancy-regex + regex-automata and their dependencies, clean | 35.5 s |
| only ropey + imara-diff + unicode-segmentation + smallvec, clean | 19.4 s |

A `quire-text` without find (regex) or file diffing (imara-diff) drops about two thirds of that.

## 2. Measurements

Machine: the WSL PC (16 cores, 31 GB), shared with other lanes (load average 8–13 during the runs, two build
slots busy). Release build, one thread. Times are `Instant` per op: WSL's clock has a 100 ns granularity (every
value is a multiple of 100 ns) and 20–30 ns of overhead, so values under 1 µs are coarse; p99 includes
interference from other processes. Heap bytes come from a counting global allocator (live-bytes delta, not RSS).
One full run is reported (`target/release/tc-eval`); an earlier run of the same code gave the same picture, with individual medians up to about 40% apart.

Models:

- **egui**: egui 0.37 `TextEdit`'s state: `String` + `CCursorRange` + `Arc<Mutex<Undoer<(CCursorRange, String)>>>`.
  The undoer (egui's `util/undoer.rs`, copied into the bench) is fed `(cursor, text.to_owned())` before and after
  the events of every focused frame, as `text_edit/builder.rs` does (lines 1215 and 1519). Word stops come from
  svelters-egui's `native/words.rs` (Pango rules), as FX1 applies them.
- **quire D+V**: `Document` + one `EditorView`, edits through `Document::execute` (the path egui_quire uses).
  "default config" is `EditorConfig::default()`; "field config" drops the bracket, pair and comment tables and
  auto-indent (the config a field would get).

### 2.1 Memory per field

Inline sizes: `String` 24 B, egui field 72 B, `Text` 16 B, `Document` 984 B, `EditorView` 472 B,
`EditorConfig` 384 B; `Selection` 48 B, `History` 272 B, `RecentChanges` 48 B. `EditorConfig::default()` is
799 B of heap in 38 allocations; the field config, 43 B in 2.

Heap bytes / allocations per field (inline size not included):

| Model | empty | 20 B | 200 B | 10 KiB |
|---|---|---|---|---|
| `String` | 0 B / 0 | 20 B / 1 | 200 B / 1 | 10.0 KiB / 1 |
| egui field, idle | 176 B / 1 | 196 B / 2 | 376 B / 2 | 10.2 KiB / 2 |
| egui field after 1 focused frame | 432 B / 2 | 472 B / 5 | 832 B / 5 | 20.4 KiB / 5 |
| egui field after 1 keystroke | 441 B / 5 | 513 B / 8 | 1,233 B / 8 | 40.4 KiB / 8 |
| `ropey::Rope` | 1,024 B / 1 | 1,024 B / 2 | 1,024 B / 2 | 12.0 KiB / 13 |
| quire `Text` | 1,024 B / 1 | 1,024 B / 2 | 1,024 B / 2 | 12.0 KiB / 13 |
| `Text` + `Selection` + `History` + `RecentChanges` | 1,024 B / 1 | 1,024 B / 2 | 1,024 B / 2 | 12.0 KiB / 13 |
| quire D+V, default config | 1,903 B / 43 | 1,903 B / 44 | 1,903 B / 44 | 13.0 KiB / 57 |
| quire D+V, field config | 1,147 B / 7 | 1,147 B / 8 | 1,147 B / 8 | 12.2 KiB / 21 |
| quire D+V, field config, after 1 keystroke | 2,611 B / 33 | 2,683 B / 34 | 2,683 B / 34 | 14.7 KiB / 49 |

With inline sizes, an idle 20 B field is 268 B in egui and about 2.6 KB in quire (field config), 3.4 KB with the
default config. `History` and `RecentChanges` allocate nothing until the first edit; the floor is ropey's 1 KiB
leaf, the per-document config, and the 1.46 KB of inline `Document` + `EditorView`. The first edit adds 1.5 KB
(the rope leaf is copied because `saved_text` still shares it, plus the open undo group and recent-changes entry).

History after 20 undo groups (20 words typed, 1.5 s pause after each; heap of the whole field):

| Model | 20 B | 200 B | 10 KiB |
|---|---|---|---|
| egui (whole-text snapshots, up to 100) | 3,853 B | 7,873 B | 233.2 KiB |
| quire D+V, `recent_changes_window` 64 (default) | 24.3 KiB | 24.3 KiB | 36.4 KiB |
| quire D+V, `recent_changes_window` 4 | 8,571 B | 8,571 B | 20.5 KiB |

The recent-changes window (one change set + line edits per keystroke, 64 kept) is two thirds of quire's history
memory on short text. A single-view field needs very few of them.

### 2.2 Creating 10,000 fields (ms, median of 5, drop included)

| Model | empty | 20 B | 200 B | 10 KiB |
|---|---|---|---|---|
| `String` | 0.03 | 1.03 | 0.72 | 121.5 |
| egui field | 1.18 | 1.84 | 2.31 | 122.5 |
| quire `Text` | 5.09 | 12.2 | 14.3 | 379.2 |
| quire D+V, default config | 74.0 | 88.8 | 97.5 | 638.8 |
| quire D+V, field config | 28.0 | 54.2 | 55.9 | 517.0 |

Per field: 0.18 µs (egui) against 5.4 µs (quire, field config) for 20 B. Building 10,000 at once costs 2–5 frames.

### 2.3 Per keystroke (median / p99)

Caret at the end, one op per focused frame (egui: feed, op, feed); the field is rebuilt (untimed) every 32 ops so
its size stays near the nominal one.

| Op | Model | 20 B | 200 B | 10 KiB |
|---|---|---|---|---|
| caret right (command floor) | egui | 300 ns / 500 ns | 400 ns / 500 ns | 2.40 µs / 6.40 µs |
| | quire | 4.30 µs / 13.2 µs | 4.90 µs / 26.4 µs | 4.60 µs / 15.6 µs |
| insert char | egui | 400 ns / 900 ns | 800 ns / 1.40 µs | 22.0 µs / 88.5 µs |
| | quire | 8.10 µs / 49.8 µs | 9.20 µs / 57.7 µs | 7.50 µs / 54.8 µs |
| | `String` alone, no undo feed | 100 ns / 300 ns | 400 ns / 700 ns | 18.8 µs / 66.6 µs |
| delete word left | egui | 1.40 µs / 4.30 µs | 4.10 µs / 14.2 µs | 143 µs / 269 µs |
| | quire | 19.1 µs / 81.7 µs | 24.5 µs / 110 µs | 149 µs / 290 µs |
| select word (double click) | egui | 1.40 µs / 2.10 µs | 5.00 µs / 20.1 µs | 205 µs / 363 µs |
| | quire | 12.2 µs / 57.4 µs | 17.1 µs / 73.1 µs | 132 µs / 246 µs |
| focused frame, no input | egui (2 text clones) | 800 ns / 900 ns | 200 ns / 300 ns | 1.00 µs / 1.90 µs |
| | quire (`tick` + `sync`) | 100 ns / 100 ns | 100 ns / 100 ns | 100 ns / 100 ns |

Allocations per op on a 20 B field: caret right 3 (egui) / 4 (quire); insert 3 / 27; delete word left 8 / 28;
select word 15 / 8. quire's floor comes from the editor around the edit, not from the rope. Per command it builds
a `WordClassifier` (a `Vec<char>` of the separators) and clones the auto-closed list. Each edit computes the
inverse change set, line edits for tokens and folds, a recent-changes entry, the history record, and token,
diagnostic and fold maintenance. Grapheme and word motions copy the current line into a `String` or `Vec<char>`.
That copy is why both models cost 130–200 µs at 10 KiB on one line: svelters' `words.rs` collects the whole text
into a `Vec<char>` per call too.

### 2.4 Undo and redo of 100 typed characters (30 ms apart)

| Model | Size | Type 100 (total) | Undo steps | Undo all | Redo all |
|---|---|---|---|---|---|
| egui | 20 B | 34.9 µs | 1 | 400 ns | 300 ns |
| quire | 20 B | 723 µs | 20 | 177 µs | 136 µs |
| egui | 200 B | 76.5 µs | 1 | 1.00 µs | 500 ns |
| quire | 200 B | 1.18 ms | 20 | 280 µs | 214 µs |
| egui | 10 KiB | 2.65 ms | 1 | 5.80 µs | 3.40 µs |
| quire | 10 KiB | 826 µs | 20 | 186 µs | 142 µs |

egui groups by time (one undo point once the text has been stable for 1 s; typing never pauses here), quire by
word (Monaco). Neither is GTK's or Qt's rule (FX3).

### 2.5 Set text from a prop (`bind:value`) and read it back

Alternating two values (median / p99):

| Method | 20 B, one char | 20 B, all new | 200 B, one char | 10 KiB, one char | 10 KiB, all new |
|---|---|---|---|---|---|
| `String`: compare + replace | 100 ns / 200 ns | 100 ns / 200 ns | 100 ns / 200 ns | 1.30 µs / 3.20 µs | 1.20 µs / 2.30 µs |
| quire: replace-all `ChangeSet`, `apply_remote(Reload)` (history rebased) | 4.80 µs / 38.3 µs | 4.70 µs / 20.1 µs | 7.80 µs / 48.7 µs | 37.5 µs / 105 µs | 38.2 µs / 109 µs |
| quire: `ChangeSet::diff` + `apply_remote(Reload)` | 10.6 µs / 58.3 µs | 10.6 µs / 59.0 µs | 13.9 µs / 63.7 µs | 80.8 µs / 184 µs | 75.3 µs / 174 µs |
| quire: rebuild `Document` + `EditorView` | 4.40 µs / 8.70 µs | 4.30 µs / 7.60 µs | 4.60 µs / 8.10 µs | 19.0 µs / 67.5 µs | 24.8 µs / 76.3 µs |
| compare only, unchanged: quire `Text == &str` | 500 ns | 500 ns | 600 ns | 900 ns | 900 ns |
| compare only, unchanged: `String == &str` | 100 ns | 100 ns | 100 ns | 400 ns | 400 ns |

Reading the text for layout each frame (median):

| Read | 20 B | 200 B | 10 KiB |
|---|---|---|---|
| `String::as_str` | 100 ns | 100 ns | 100 ns |
| `String::to_owned` (the `LayoutJob` copy egui makes anyway) | 100 ns | 100 ns | 600 ns |
| quire `as_slice().as_str()` | 500 ns (Some) | 600 ns (Some) | 200 ns (None) |
| quire `to_cow()` | 500 ns | 600 ns | 2.40 µs |
| quire `to_string()` | 300 ns | 300 ns | 2.30 µs |
| quire `to_cow()` after typing 100 chars | 800 ns | 800 ns | 2.00 µs |

A rope forces no extra copy for a single-line field below one leaf (about 1 KiB): `as_str()` is `Some` there
even after edits. Above it the text is in chunks and needs one concatenation per frame, but egui's galley path
copies the text into a `LayoutJob` anyway, so the cost is that concatenation (2 µs at 10 KiB), not a second copy.
A field can also hold the galley and lay out again only when `version` changes.

### 2.6 PropertyGrid: 10,000 rows, 3 focused (one keystroke each per frame)

Every row's value is read (FNV hash, standing in for the galley-cache key) and compared with its prop each frame.

| Model | Heap, 10k rows | Build 10k | Frame: idle rows | Frame: 3 focused | Frame total |
|---|---|---|---|---|---|
| egui field | 2.5 MB | 1.43 ms | 690 µs | 5.7 µs | 696 µs |
| quire D+V, default config | 32.8 MB | 56.9 ms | 8.97 ms | 92 µs | 9.06 ms |
| quire D+V, field config | 25.4 MB | 34.7 ms | 8.89 ms | 94 µs | 8.99 ms |
| hybrid: `String` idle, 3 promoted to quire D+V | 0.34 MB | 1.02 ms (+0.23 ms to promote 3, cold) | 306 µs | 84 µs | 390 µs |

A virtualised grid reads only its visible rows, so the per-frame column overstates what a real grid pays. Memory
and build time are what decide: quire everywhere means 2.5 KB and 3.5–5.7 µs per row before anything is focused.

## 3. What "advanced features everywhere" would give, and what is missing

| Feature | Value in plain inputs | Verdict |
|---|---|---|
| Multi-cursor, column selection | none natively (GTK, Qt, AppKit, WinUI fields have one caret); breaks pin 2's selection checkpoints | opt-in for multi-line `text-area`s only |
| Smart word movement | VS Code's classifier differs from Pango and Qt (`snake_case`, `3.14`, Japanese script breaks): FX1 pin 1 would drop from 72/72 | TC0 needs pluggable `WordRules` (move `native/words.rs` in, byte offsets) |
| Rich undo | word-level groups beat egui's one time-based group; GTK and Qt each have their own rules (FX3 pin 4) | TC0 needs pluggable `UndoRules` (Monaco, Gtk, Qt) and a cap |
| Find / replace | not in a field; useful in a large text area or code field | stays in quire-core |
| Auto-close pairs, surround, comments, indentation | harmful in a name field (typing `(` inserts `)`) | stays in quire-core |
| Transactions, versions, change sets, anchors | yes: `bind:value` without losing the caret, AccessKit text, remote edits, validation spans | TC0 core |
| IME preedit model | yes | TC0 core (add GTK hex entry as a preedit source) |

Missing for a UI text field (all absent from quire-core at the pin): single-line mode and paste-newline policy;
secret (password) mode with word motions to the ends and no copy; max length; input filter; read-only with copy;
external set that resets history (GTK `set_text`, Qt `setText`); an undo-stack cap; platform word rules;
GTK/Qt undo rules; backspace by code point for Pango's scripts; X11 PRIMARY ownership on select (host side, a
host request); placeholder and password bullets (painting, svelters side, ARCH-WIDGETS §4.1).

## 4. Recommendation

| Option | Memory, 10k idle 20 B fields | Build 10k | Keystroke (20 B) | Verdict |
|---|---|---|---|---|
| (A) quire-core everywhere | 25–33 MB | 35–57 ms | 8 µs, 27 allocs | no: 10x memory, frame hitches when tables build, labels carrying an editor |
| (B) quire-core for inputs, light path for labels | as (A) for every input | as (A) | 8 µs | no for editable tables; the labels half is right |
| (C) lazy hybrid over quire-core as it is | 0.34 MB | 1 ms | 8 µs once focused | workable now; keeps the editor's per-document weight and its compile cost |
| **(D) smaller TC0 (`quire-text`) with a `String` backend for small texts, created lazily as in (C)** | ~0.3–0.5 MB idle (estimate) | ~1–3 ms (estimate) | target ≤ 2 µs, ≤ 8 allocs | **yes** |

The (D) row's figures are targets, not measurements. TC0 should land with this bench extended by a `quire-text`
row and pass these gates on the WSL PC: an idle 20 B `TextDoc` + `TextView` at most 512 B (inline + heap) and 1 µs
to create; insert at most 2 µs median and 8 allocations at 200 B; no regression against the quire numbers above
at 10 KiB.

**The backend under quire-text's API for small fields** (the coordinator's question): a flat buffer,
`Arc<String>` with copy-on-write (`Arc::make_mut`), so `Text::clone` and snapshots stay O(1) as quire's
`Snapshot` contract requires. Its `as_str()` is always `Some`, edits are `String::replace_range`, and an empty field
costs one small allocation instead of ropey's 1 KiB leaf. Switch to ropey when the text grows past 64 KiB (or when
a document is created from a file), and never switch back while the document lives. quire's editor documents
start as ropes, so their behaviour does not change. ropey stays behind a cargo feature that quire-core enables.

**svelters side (lazy, from C):** an unfocused `text-edit` holds only its value string (it already mirrors the
prop). It creates its `TextDoc` + `TextView` on focus, pointer-down or the first programmatic selection, and keeps
it while the node lives, because GTK and Qt keep undo across focus changes. A prop write with `ResetHistory` and no
focus can drop it again. Selectable labels never create one. They keep a `Selection` and call quire-text's pure
functions (`word_at`, `next_word`, `prev_grapheme`, ... over `&str` with a `WordRules`).

### 4.1 API surface TC0 should export to svelters

ARCH-WIDGETS §4.2–4.5 (on `research/arch-widgets`) has the shape; the numbers add these requirements:

- `Text`: `len`, `len_chars`, `line_count`, `line(i)`, `slice(r)` (chunks), `as_str() -> Option<&str>`
  (always `Some` on the flat backend), `to_string`, `char_at`, conversions byte ↔ char ↔ UTF-16, `line_of`,
  `line_start`, `grapheme_next/prev`, `PartialEq<str>`. No backend type in the API.
- `TextDoc::new(&str, &'static DocRules | Arc<DocRules>)`: no config copy per document. No history, recent-changes
  or saved-text allocation until the first edit. A recent-changes capacity per document (fields: 4).
  `text`, `version`, `apply`, `apply_remote`, `replace_all(text, ExternalSet::{ResetHistory, Undoable})`,
  `close_undo_group`, `tick(now_ms)`, `can_undo/redo`, `undo_mode`, `anchor/resolve_anchor`.
- `TextView::new(&TextDoc)`: selection, `selection_version`, `sync`, `set_selection`, preedit, `composing`,
  `set_focused`. Small: no find, folds, scroll or session state (those stay in quire-core's `EditorView`).
- Field commands (`EditCommand`, ARCH-WIDGETS §4.5), resolved with a reused word classifier and line access that
  does not copy the line. `Applied` / `Refusal` / `UndoGroupClosed` as in quire-core.
- Rules: `DocRules { undo: UndoRules (Monaco | Gtk | Qt), undo_pause_ms, undo_mode, max_undo }`, `FieldRules
  { lines, max_chars, secret, read_only, words: WordRules (VsCode | Pango | Qt | Custom), word_move_collapses,
  backspace, filter }`, all `#[non_exhaustive]`.
- Pure functions for read-only text: word and grapheme stops over `&str` with a `WordRules`.
- Dependencies: smallvec, unicode-segmentation, unicode-general-category (Pango rules), ropey behind a feature.
  No regex, no imara-diff: `replace_all` trims the common prefix and suffix instead of diffing.

### 4.2 quire-side refactor (cadatomic's, macbook-cadatomic)

1. `Text` gets an internal backend enum (flat `Arc<String>` and rope) with the threshold above; ropey behind a
   feature. The public API is already backend-free.
2. Split `Document` into `TextDoc` (text, version, history, recent changes, rules) in quire-text and quire-core's
   `Document` wrapping it (tokens, diagnostics, folds, saved text and dirty state, `EditorConfig`, pending work,
   large-file state). Split `EditorView` likewise (`TextView`: selection, version, sync, preedit, focus;
   quire-core keeps folds, find, scroll, reveal, auto-closed pairs, sessions).
3. Make `History` and `RecentChanges` lazy, the recent-changes capacity per document, and the undo stacks capped.
4. Take the per-command allocations out of the shared paths: cache the `WordClassifier` per document, and give
   the motion and word ports a line accessor that borrows `&str` (`as_str` on the flat backend, chunks on the rope)
   instead of `Model::line_string` / `line_chars`.
5. Make word and undo rules pluggable (`WordRules`, `UndoRules`). svelters' `native/words.rs` (FX1) and FX3's undo
   models move into quire-text with their tests. quire-core keeps VS Code/Monaco as its defaults.
6. Add `FieldRules` (single-line, max chars, filter, secret, read-only) and `replace_all` with the two external-set
   modes.
7. Keep find (regex-automata, fancy-regex), language rules (`lang.rs`), commands beyond the field set, and
   `ChangeSet::diff` (imara-diff) in quire-core. quire-text then builds without the regex stack (about 35 s of
   the 52 s clean build here).
8. Keep `tests/monaco.rs`, `laws.rs` and `robust.rs` running against quire-text's types (the change-set laws and
   the VS Code port are what make the extraction safe), and add the bench row above as the extraction's gate.

## 5. Not run, caveats

- Windows and macOS were not measured, and neither was the macbook. The numbers are from one contended WSL run;
  the gates in §4 should be re-measured there when quire-text exists.
- Painting was not measured (galley layout, selection and caret painting): it is the same for every model.
- egui's `TextEdit` was modelled, not run: its state and undoer are copied exactly, but its event loop and galley
  work are not included.
- quire-text's (D) numbers are estimates until the crate exists.

## Reproduce

```sh
# from the svelters checkout root; needs access to github.com/akiselev/quire (or cadatomic's cargo git cache)
tools/dev/cargo-slot cargo build --release --manifest-path plans/research/tc-eval/Cargo.toml
plans/research/tc-eval/target/release/tc-eval            # all sections; or one: memory|create|keys|allocs|undo|history|settext|read|grid
rm -rf plans/research/tc-eval/target
```
