# POL4-ROWS: the 24 px trial reaches lists and trees (batch polish-b4, lane rows)

**Branch:** `ci/pb4-rows` (base `f790130`: master `a8937ea` plus the lane brief).
**Brief:** `plans/cloud/polish-b4/BRIEF.md`, `LANE-rows.md`.
**Status (2026-10-28):** done; CI under "CI". Questions: none (two judgement calls recorded under "Kinds with their
own row constant" and "Outside the lane's file list").

## What changed

1. **An unset `row-height` follows the theme** for `virtual-list` and `tree-view` (`views/virtual_list.rs`
   `apply_row_height`, `views/tree.rs`). Unset, a row is `Theme::row_height_for(font-size)`: the density's `row-h`
   grown past the base text size (SV83's rule, as `auto-row-height` and the table use), so compact 20, default 24,
   comfortable 28. A written `row-height` wins; `auto-row-height` keeps its meaning (the type row, at least a written
   `row-height`). Without a theme the prop defaults stay (32 list, 24 tree `TREE_ROW_HEIGHT`). The tree now notes the
   type scale in its `layout_style` and applies it before each realise, as the list does.
2. **`<ListView>`/`<TreeView>` write `row-height` only when the app gives one** (`packages/native-runtime/src/lists.js`
   `rowHeightEffect`). They used to write their defaults (32/24) always, which counted as "written" and hid the theme.
   Removing a given `row-height` later writes `null`. Other backends see no write and use their native defaults, which
   are the same numbers (svelters-egui 32/24, svelters-gtk 32). The six list fixtures' `expected.oplog` lose their
   `set #N row-height 32` line, and `lists.test.mjs` its expected op.
3. **Stories:** `tree-view` (all six variants) dropped `row-height={22}`; `virtual-list` `plain`, `empty` and `rtl`
   dropped `row-height={24}`. `rich-rows` keeps `row-height={40}` (two-line rows need it), said in the story.toml
   notes; the tree's notes say no variant sets it. No other story pins a row height.
4. **Docs:** the surface docs of both `row-height` props (`svelters-skelter` `surface/tier1.rs`, `skelter.json`
   regenerated with `SVELTERS_UPDATE=1 cargo test -p svelters-skelter --test spec`), the `ListProps` doc comment, and
   `packages/lists/src/index.d.ts`.

## Kinds with their own row constant

| Kind | Source | Default density | Decision |
|---|---|---|---|
| `virtual-list` | `row-height` unset | 32 → 24 | routed through `row-h` (above) |
| `tree-view` | `row-height` unset (`TREE_ROW_HEIGHT`) | 24 → 24 (stories 22 → 24) | routed through `row-h` |
| `table`, `select`/`combo-box` options | `row-h` already (POL4-SMALL) | 24 | unchanged; renders show no change |
| tab overflow menu rows (`tabs/mod.rs`) | text line + 2 × `pad_y` (≈26) | ≈26 | left: it is a menu, sized as the `menu` entries (26), not a list row; already above 24 at every density |
| `select` no-theme fallback (`views/select.rs` `unwrap_or(22.0)`) | only without a theme | n/a | left: the option's UA height is `Token("row-h")`, so every themed app follows density; the fallback never shows in an app |

## Outside the lane's file list

`packages/native-runtime/src/lists.js` (+ its test, `packages/lists/src/index.d.ts`), the six `fixtures/*list*`
oplogs, `surface/tier1.rs` and `skelter.json`: needed because the components' always-written default made item 1
invisible. Additive and small; no other lane in the batch owns them.

## Tests

- `skelter-widgets` `views::virtual_list::sv83::unset_row_height_follows_the_density` (new: list and tree at compact
  20, default 24, comfortable 28, a written value wins) and `sv83_auto_rows_follow_the_type_scale` (updated: unset now
  follows the theme; written wins). `cargo test -p skelter-widgets --lib views::`: 154 passed.
- `svelters-skelter` `--test spec` (regenerated), `--test host_document --test tier1 --test vlist --test
  oplog_replay`: all pass (8, 1, 14, 5); no snapshot moved, nothing re-blessed.
- `packages/native-runtime` `lists.test.mjs`: 10 passed. Fixtures `virtual-list`, `lists-recycle`, `lists-stress`
  and their `-qt` twins (`pnpm test` in each): pass.
- Not run: the full workspace, other backends' crates (CI).

## Before/after (dark-modern, `shot --density <d> --diff`, master built in a worktree)

Images under `/tmp/claude-0/-home-user-svelters/b857fdc6-78ad-5d32-808f-505920f1c4de/scratchpad/img/`:
- `tree-view-expanded-default-after.diff.png`: rows 22 → 24, twisties stay centred.
- `tree-view-expanded-comfortable-after.diff.png`: rows 22 → 28.
- `virtual-list-plain-compact-after.diff.png`: rows 24 → 20 (default: no change, 24 both).
- `table-basic-*` and `combo-box-open-*` at all three densities: no change (they already followed `row-h`).

## CI

All six workflows green on `04bfbdc` (code `99bd15b` + the record, `origin/master` `a8937ea` included): js
`40728397038`, linux-gtk `40728398123`, linux-qt `40728396920`, macos-appkit `40728397241`, skelter `40728397025`,
windows-winui `40728397033`. This CI note is docs-only (no rerun).
