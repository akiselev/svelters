# U2: style-engine prerequisites for utility classes (E3, E5, E7)

Design: `plans/research/UTILITY-CSS.md` §1.2 (F2, F8, F9), §5 (E3, E5, E7), §7 (lane U2 and its
acceptance tests). Lane U2 on the WSL PC, branch `ci/u2`. Each item also stands on its own: the part
index speeds up the theme's part rules today, the longhands are standard CSS, and the platform
feature lets sheets differ per platform.

## What landed

| Item | What | Where |
|---|---|---|
| **E3** part-rule index | Selectors ending in a pseudo-element no longer sit in one list. `::part()` rules are bucketed by the selector's first part name, `::placeholder` and `::selection` rules have a bucket each, and inside each bucket rules are filed by the host compound's id, else first class, else tag, else universal (the same `Buckets` type as the main index). A part's computation collects only the buckets of its own names and of the element's id, classes and tag (and `style_as` tag). `has_pseudo_rules` is a map lookup plus the P10 name-set test. `:is()`/`:where()` of one selector (what nesting's `&` becomes: `slider { &::part(x) }` is `:is(slider)::part(x)`) now buckets by its subject compound, in both the part index and the main index, so nested rules are no longer universal candidates. New `Stylist::pseudo_candidates(el, pseudo) -> Vec<RuleId>` lists what a pseudo computation tries, for tests and stats. | `svelters-style/src/cascade.rs` |
| **E5** transition longhands | `transition-property` (`none` or a list of `all` and names), `transition-duration`, `transition-timing-function` and `transition-delay` (comma lists). `transition` is now a shorthand on tables that have the longhands (egui): it expands, after `var()` substitution, into the four longhands in the cascade's winner pass, as OW-CSS's border shorthands do. So a later longhand changes only its part, and a later shorthand resets all four. After the cascade, the `transition` value (`ComputedValue::Transitions`, what `Transitions<K>` and `StyleView::transitions` read) is assembled from the longhands per CSS Transitions 1 §2: `transition-property` sets the count, the other lists repeat or are cut, and unset ones take `all 0s ease 0s`. Nothing set gives no value, as before. The longhands and `transition` itself never transition. GTK keeps `transition` and the longhands as raw `via: css` text, untouched. | `values.rs` (`ValueType::{TransitionProperty, TimeList, EasingList}`, `ComputedValue::{Names, Times, Easings}`, `expand_transition`, `combine_transitions`), `shorthand.rs`, `computed.rs`, `transition.rs`, `table.rs` |
| **E7** platform media feature | `@media (-sv-platform: macos \| windows \| linux)`, and `(-sv-platform)` (true on those three). `MediaEnv::platform: Platform` (`Macos`, `Windows`, `Linux`, `Other`). Its default is `Platform::current()` (the build target), for hosts that never set it. On egui, `update_media` sets it every frame from `native::Policy::platform` (new field, `Option<Platform>`: GTK and Qt rows `Linux`, the Windows row `Windows`, the Mac row `Mac`, the egui row `None` = the context's OS). So a test emulates a platform with `native::override_policy(ctx, \|p\| p.platform = Some(Platform::Macos))` or `native::set(ctx, Native::Mac)`, and a flip restyles like any media change. | `media.rs`, `svelters-egui/src/native/mod.rs`, `svelters-egui/src/style.rs` (`update_media`) |

Generated: `ENGINE_PROPS` gained the four longhands (decl types `transition-property`, `time-list`,
`easing-list`); `metadata/semantics/egui.json` (`SVELTERS_UPDATE=1 … --test spec`), then
`tables.generated.rs` and `props.generated.js` (`gen_props.py`, whose `RS_TYPE` gained the three
types), then `gen_reference.py` (no page changed: the widget pages don't list engine properties).
`egui.d.ts` did not change. Docs: `docs/egui/css/own-widgets.md` (longhands; the platform feature).

OW5a (`ci/ow5a`) calls `has_pseudo_rules(&PseudoElement::part(p))` and `compute_pseudo_at(Forced{…},
…, Some(at), …)`. Both keep their signatures and semantics, so its `Scene::part_at` merges unchanged.

## E3 measurement

`svelters-style/tests/part_index.rs::part_index_bench` (ignored; release:
`tools/dev/cargo-slot cargo nextest run --cargo-profile release -p svelters-style --test part_index --run-ignored only --no-capture`).
A sheet of 331 part rules: 30 part names × 10 host classes, 30 `slider::part(pN):hover` and one
host-free `::part(p7)`. Medians of 301 × 100 calls on the WSL PC, under a load average of 13–19 from
other lanes, so the absolute numbers are noisy; the before runs used master's `cascade.rs` with the
rest of the branch.

| Call (part `p7`) | Before (2 runs) | After (3 runs) |
|---|---|---|
| `matched_pseudo_rules`, matching host `slider.host3` | 10.0–11.9 µs | 0.81–0.84 µs |
| `matched_pseudo_rules`, other host `box.other` | 10.1–10.2 µs | 0.40–0.41 µs |
| `compute_pseudo`, matching host | 21–34 µs | 13–17 µs |
| `compute_pseudo`, other host | 20–30 µs | 15.6–16.2 µs |
| `has_pseudo_rules`, a used part | 1.45–1.47 µs | 0.06–0.09 µs |
| `has_pseudo_rules`, an unused part | 4.3–5.0 µs | 0.04–0.06 µs |

Matching is about 14× (matching host) to 25× (other host) faster, and `has_pseudo_rules` 25–100×.
What is left of `compute_pseudo` is `compute()` itself: a walk over the whole egui table for one
part (E8/E9's territory, measured by U7).

## Tests

- `svelters-style/tests/part_index.rs` (new): `part_rules_match_through_the_bucket` (the acceptance
  test: a part rule matches through its bucket, and rules on another part name or another host
  class are never candidates; multi-name parts found once through any name; placeholder and
  selection buckets; nested `&::part()` with P1 positions; `has_pseudo_rules`; pseudo rules never
  candidates for the host), `a_big_sheet_tries_one_bucket` (3 of 331 tried), and the ignored bench.
- `svelters-style/tests/ow_css.rs::transition_longhands_cascade_with_the_shorthand` (the acceptance
  test, extending OW-CSS's transition tests): longhand after shorthand and the reverse, list
  repetition and cutting, initial values, `transition-property: none`, utility-style composition
  in either class order, `var()` in both, the longhands' own values, five bad-value errors (a
  negative delay is fine), and `Transitions<K>` running on the assembled list.
- `svelters-style/tests/cascade.rs::platform_media_follows_the_env` and
  `media.rs::tests::platform_feature` (the acceptance test: `@media (-sv-platform: macos)` follows
  `MediaEnv`, the flipped rules are reported).
- `svelters-egui/tests/ow_css.rs`: `transition_longhands_compose_across_classes` (a hover eases
  over the longhand duration from a second class, in kittest's virtual time) and
  `platform_media_follows_the_native_policy` (policy override, `Native::Gtk`, `Native::Mac`,
  `Platform::Other`).

Run locally: `cargo nextest run -p svelters-style` (63 passed), `-p svelters-egui --test ow_css
--test style --test spec` (16 passed, again after merging master with OW5a1 and ICR1), OW5a's
`--test vlist` after the merge (10 passed; `list_view_runs_on_egui` needs the `pnpm build` fixture
bundle and was not run), the bench, `cargo clippy -p svelters-style --all-targets` (clean). Not run
locally: the full workspace, the JS packages' tests (no `node_modules` in the worktree;
`gen_props.py --check` is clean), other backends. CI is the gate.
