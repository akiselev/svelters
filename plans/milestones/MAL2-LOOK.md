# MAL2-LOOK: tokens through the cascade, a user origin, cheap token changes (SV67–SV71)

Lane `S-B look` of cadatomic's MAL2 batch, branch `ci/mal2-look`, base master `4e9f4b62` (merged
`3096ff6f` during the lane). Coordinator: cadatomic's batch coordinator session. Every seam is
generic: no app names, token families or concepts. User-facing docs:
`docs/skelter/tokens-and-origins.md`.

**Status:** done; CI green on `e2ba4592`, waiting for the coordinator's merge.

Order followed: SV70 and SV67 first, then SV68, SV71, SV69 (the brief's order). The request numbers
are the brief's; an earlier draft of the batch's design numbered the same requests one lower.

## What landed

### SV70: `Origin::User`, `revert`, `revert-layer` (`svelters-style/src/{cascade,computed}.rs`)

- `Origin::User`. Ranks, lowest first: normal theme 0 < author 1 < user 2 < inline 3; `!important`
  author 4 < inline 5 < theme 6 < user 7. User rules beat author rules (the app) whatever their
  specificity; inline (code-computed geometry) beats user normal; a user `!important` beats all.
  `matched_rules` lists user rules after author rules.
- `revert` rolls back to the cascade without the declaration's origin (inline counts as author):
  author/inline → theme, user → author and inline, theme → `unset`. `revert-layer` rolls back to
  the earlier layers of the same origin and importance, then on to the origin below. Shorthands and
  custom properties roll back too. A declaration's whole value must be the keyword; one produced by
  `var()` still acts as `unset`.
- Cost: `compute` takes a fast path (last declaration wins, as before) unless a matched declaration
  is a rollback keyword; only then does it resolve per property from the top.
- skelter-core: `Document::add_sheet_with_origin`, `Document::add_parsed_sheet_with_origin`;
  `skelter_core::Origin` re-exported.
- Tests: svelters-style `cascade::user_origin_ranks`, `cascade::revert_and_revert_layer`;
  skelter-core `tests::style::user_sheets_beat_author_rules_but_not_inline`.

### SV67: UA tokens read the computed `--sv-*` (`skelter-widgets/src/paint/mod.rs`)

- `StyleView` resolves `UaValue::Token(name)` through the view's computed custom property
  (`--sv-<name>`, or the name itself when it starts with `--`); a part view without its own style
  reads its host's. The typed `Theme` is the fallback (no token sheet, `StyleView::ua_only`).
  Colours, lengths, numbers and `box-shadow` tokens all go this way.
- Parse cache: per thread, keyed by the address of the shared value `Arc` (the cache holds a clone,
  so an address isn't reused while cached); 4,096 entries, then it starts over.
  `ComputedStyle::custom_shared` (new) exposes the shared text.
- `paint::record_token_reads(f)` reports the tokens read during `f`; the widget host uses it for
  SV69.

### SV68: component tokens as `var()` (`skelter-widgets/src/theme/mod.rs`)

- `Theme::css()` and `default_tokens_css()` write each L3 component token that follows its role as
  `var(--sv-<role>)`; an overridden component is written as its literal. `Theme::sheet_tokens()`
  is what the sheet declares; `tokens()` and the typed views keep resolved values.
- `Theme::sheet_changes(&old)`: the token edits from `old`'s sheet to this one's.
- Test: `theme::tests::component_tokens_stay_aliases_in_the_sheet`.

### SV71: token listing and override sources (`skelter-widgets/src/theme/mod.rs`)

- `TokenInfo::doc` (new field) and `theme::token_doc(name)`: the role, component and metric tables'
  doc comments (the colour and metric macros now keep them as data), plus short docs for elevation,
  popup/window shadows and the font families. The listing already had name, tier, type, the value in
  each built-in set and per density.
- `Theme::with_source(source, overrides)`: tags overrides with a host-chosen name;
  `Theme::override_source(name)`; `Theme::explain(name) -> Option<TokenOrigin>` with
  `Override { source }`, `Follows(token)` (role ← `--vscode-*` key, component ← role) or
  `Builtin`. `TokenOrigin` is `#[non_exhaustive]`. Tags survive `with_scheme` and the other recipe
  changes; an untagged override of the same name clears its tag.
- Test: `theme::tests::overrides_carry_their_source`.

### SV69: tracked custom-property restyle, `set_root_tokens` (`skelter-core`, `svelters-style`)

- svelters-style: each rule records the custom properties its declarations reference and whether it
  declares any (`CustomDeps`); `Stylist::compute_with_deps`, `compute_pseudo_with_deps`;
  `ComputedStyle::changed_customs` (sorted diff, pointer-equal first) and `inherit_customs_from`;
  `Stylist::set_custom_values(sheet, edits)` edits a sheet's custom declarations in place (no index
  rebuild), adding a new name to the sheet's last rule that declares custom properties.
- skelter-core `restyle`: a node whose custom map changed passes the changed names down instead of
  forcing its subtree (SV65's behaviour). A descendant restyles when its or its parts' declarations
  reference a changed name, it declares custom properties, or its kind reads one
  (`Kind::reads_custom_property`, new, default `true`); otherwise it only takes the parent's new map.
  A restyled node whose kind reads a changed name re-measures and re-records.
- skelter-widgets' host answers `reads_custom_property` from the UA tokens the widget read in
  `layout_style`, `measure`, `prepare`, `record` and the chrome hooks (it only grows: a token read in
  one state stays a dependency).
- `Document::set_root_tokens(sheet, edits)`: edits in place, restyles only the nodes the edited
  rules match, then the tracking above. `skelter_widgets::host::set_theme_tokens(doc, sheet, theme)`
  edits the sheet with `sheet_changes` and replaces the service (a scheme, contrast or density change
  also repaints all, as `set_theme` does).
- `FrameStats::restyled` (new) and `Document::style_rebuilds()` for tests and stats.
- Tests: skelter-core `tests::style::a_custom_property_change_restyles_only_its_readers` (4 of 24
  nodes restyle; 2 for a change nobody reads), `tests::style::set_root_tokens_edits_a_sheet_in_place`;
  skelter-widgets `host::tests::token_changes_reach_their_readers_through_the_cascade` (a UA token,
  an aliased component, only readers re-record, no index rebuild, a scheme change through
  `set_theme_tokens`).

### Coordinator's extra: a one-side border keeps its colour (`skelter-core/src/style/cascade.rs`)

- Reported: `border-bottom: 1px solid var(--sv-border)` computed a transparent `border_color`. The
  engine computed `border-bottom-color` right, with `var()` and with a literal; the loss was in
  skelter-core's `ComputedBox`, which has one border colour and style and read them from
  `border-color`/`border-top-*` only. Now the box takes the first drawn side's (top, right, bottom,
  left) style and colour, falling back to `border-color`. A side with `none`/`hidden` style gets
  width 0 on its own (before, a `border-top: none` zeroed every side). Widget painting
  (`StyleView::border_sides`) was already per side.
- Test: `tests::style::a_one_side_border_keeps_its_colour`.
- Still one colour per box: different colours on different sides of a core-painted box need a
  per-side border in the display list; not asked for.

## Decisions taken in the lane (recorded for the coordinator)

- **User `!important` above theme `!important`.** The brief says user `!important` is top. The batch's
  design note kept theme `!important` at the top; the brief wins. Theme `!important` still beats author
  and inline `!important`, so no existing test changed.
- **User normal below inline**, per the brief (the design note had it above inline).
- **A node that declares custom properties restyles on any inherited custom change** (its map must be
  rebuilt). Recomputing only its custom map would save more; not needed for the measured case.
- **`set_theme` is for documents without a token sheet.** With a sheet, UA tokens read the sheet first,
  so a service-only change no longer reaches them; such hosts use `set_theme_tokens` (or edit the
  sheet themselves). This is the intended single reader path; a host that swaps the sheet and the
  service together, as before, keeps working.

## Carry-over

- One border colour and style per core-painted box (`ComputedBox`): different colours on different
  sides need a per-side border in the display list.

## Not done

- Revert through `var()` substitution acts as `unset` (rare; CSS allows it).
- `Stylist::winners` and `cascaded` (inspector paths) still list declarations last-wins without
  applying rollbacks.
- No timing run of the 3,600-node theme-change experiment on this branch; the tests count restyled
  and re-recorded nodes instead. A measurement belongs with the consumer's own scenario.

## Lines for cadatomic's `docs/plan/04-kernel-requests.md`

- SV67: done (svelters `ci/mal2-look`): UA `Token(name)` reads the computed `--sv-<name>` first, parse cached; the typed theme is the fallback without a token sheet.
- SV68: done: component tokens are `var()` of their role in `Theme::css()`; typed views resolved; `Theme::sheet_tokens`, `Theme::sheet_changes`.
- SV69: done: tracked custom-property restyle (`Kind::reads_custom_property`, `FrameStats::restyled`); `Document::set_root_tokens(sheet, edits)`; `host::set_theme_tokens(doc, sheet, theme)`.
- SV70: done: `Origin::User` (theme < author < user < inline; user `!important` top), `revert`, `revert-layer`, `Document::add_sheet_with_origin`.
- SV71: done: `token_manifest()` with `doc`, `token_doc`; `Theme::with_source`, `override_source`, `explain` (`TokenOrigin`).

## CI

- `9ddc1a1f` (SV67–SV71 with master `3096ff6f` merged): all green — js 40462895886, linux-gtk
  40462895915, linux-qt 40462895807, macos-appkit 40462895801, skelter 40462895861, windows-winui
  40462895937.
- `e2ba4592` (+ the one-side border fix): all green — js 40465010323, linux-gtk 40465010256,
  linux-qt 40465010325, macos-appkit 40465010371, skelter 40465010374, windows-winui 40465010397.
- Not run locally: the full workspace tests and conformance (CI ran them); no timing run.
