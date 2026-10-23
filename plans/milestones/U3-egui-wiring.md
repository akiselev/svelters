# U3: utility classes on egui

Lane U3 on the WSL PC, branch `ci/u3` from master 5aefc137. Design: `plans/research/UTILITY-CSS.md`
§4.1–§4.6 and §7 (U3's row and acceptance tests), with the user's decisions in §9 taking precedence:
run-time generation in the host engine, utilities below component CSS and opt-in with an optional prefix,
our tokens only, typed values, `part-<name>:`. Engine API: `plans/milestones/U1-utility-resolver.md`
("API for U3").

## What landed

| Item | What | Where |
|---|---|---|
| `apply_classes` → `ensure_utilities` | With utilities on, a node's class list first goes through `Stylist::ensure_utilities` (one batch per class change; known and rejected classes cost one lookup), so the class change's reach sees the new selectors. The node retains the classes it gained and releases those it lost; a destroyed node releases its classes (`SceneStyle::forget`). `collect_utilities()` runs once per restyle pass (`restyle_marked`). When a generated rule adds a state the journal observer doesn't follow (`checked:bg-x`), the observer is recreated. Issues are printed once per class (`[svelters-egui/utilities]`): errors always, warnings and notes in debug builds. | `svelters-egui/src/style.rs` |
| `Utilities`, `EmbedConfig::utilities` | `svelters_egui::Utilities` (`#[non_exhaustive]`, builder style): `Utilities::on().prefix("tw").strict_tokens(true).high_water(n)`, `Utilities::off()` (the default). `EmbedConfig::utilities` + `with_utilities`; `Embed::new` applies it. `EguiBackend::set_utilities` changes it at run time: when `enabled`/`prefix` change, the engine drops every rule and the live nodes' classes are resolved again (and retained), and every node restyles. `EguiBackend::utilities()`, `utility_stats()`. | `style.rs`, `embed/mod.rs`, `lib.rs` |
| Token names from the theme | `Theme::utility_tokens()` (public): every `--sv-*` token of the theme, typed as the token manifest types them (`token_type`, now `pub(crate)`), so overrides like `--sv-cad-rail` are values. `set_theme` installs them; with no theme, the default theme's (both schemes). `set_tokens` (SV5) adds its `--sv-*` names. When the names change, classes rejected before are resolved again on live nodes (a node that carried `bg-cad-panel` before the token existed picks it up). | `theme/mod.rs`, `theme/backend.rs`, `style.rs` |
| `@layer components, utilities` | `theme::LAYERS` and `base.css` declare `sv.tokens, sv.base, sv.affordance, components, utilities`, so the layer order is fixed before any sheet loads and the first generated utility appends without a rebuild. Layers are global names; the theme's are at theme origin, utilities at author origin below unlayered component CSS. | `theme/mod.rs`, `theme/base.css` |
| `imm` `.class()` | `El::class` already existed; `Sv::computed` now generates the utilities of the chain's classes before computing a style it hasn't cached (`Scene::ensure_virtual_utilities`). Virtual elements hold no reference, so past the high-water mark the GC may drop such a rule; the next computation after `imm`'s cache drops (any full restyle) generates it again. | `imm/mod.rs`, `style.rs` |
| GTK/Qt warning | In the JS runtime's non-engine class path (GTK CSS, QSS): the first class that is unmistakably a utility (a variant prefix `hover:…`, a bracketed value `w-[120px]`, or a spacing/size/opacity namespace with a number `p-4`) warns once per process: `utility classes render on egui only: 'hover:bg-list-hover' is a plain class on gtk`. Semantic classes (`row`, `text-input`, `top-bar`, `border`, `flex`) never match. One place covers both toolkits (Qt's class path is generated C++). | `packages/native-runtime/src/style.js` |
| Gallery page | `fixtures/gallery-egui/src/pages/50-utilities.svelte` ("utilities"): spacing, colours, radius, a dynamic `p-{n}`, `hover:`, `shadow-2`, `part-thumb:`/`part-fill:` on a slider. `svelters-egui-run --utilities[=<prefix>]` turns utilities on for an app run (off by default, so the page shows plain classes without it). | fixture, `bin/svelters-egui-run.rs` |
| Part styles for tests | `EguiBackend::computed_part_style(h, part)`: a node's computed `::part(name)` style. | `style.rs` |

## Choices

- **Re-resolution, not re-retention.** After a settings or token change, only classes that gained a rule
  in that call are retained by the nodes carrying them, so references never double-count (a strict-only
  change keeps the generated rules; a reset drops them and every class is retained again).
- **App bundles opt in from the runner** (`--utilities`), not from JS: a JS switch would let one root
  change class meaning for every root of the embed. cadatomic turns it on in its `EmbedConfig`.
- **Breakpoints** still use the built-in theme's px (U1's note); not re-resolved from the active theme.

## Tests

- `svelters-egui/tests/utilities.rs` (new, 9; U3's acceptance list):
  `utilities_compute_the_theme_tokens_and_follow_the_scheme` (`p-4 bg-surface rounded-md` computes the
  tokens' values, the padding lays out, a scheme switch recolours, nothing regenerates),
  `hover_utility_restyles_in_the_same_frame`, `part_variant_reaches_the_slider_thumb`,
  `utilities_sit_between_the_theme_and_component_css` (beats `base.css`, loses to a component rule, `!`
  wins), `dynamic_classes_follow_their_value` (`p-{n}`), `opt_in_and_prefix` (off: `flex` is plain;
  `tw` prefix; switching on and off at run time applies to live nodes), `app_tokens_are_utility_values`
  (SV5 tokens, late tokens, theme overrides), `imm_elements_take_utility_classes` (an `imm` element
  computes what the node does, state variants follow the virtual state),
  `a_svb_bundle_styles_like_source` (the same app from source and from a `.svb`: identical computed
  styles, before and after `p-1` → `p-2`, the component's `<style>` beating `bg-accent`).
- `svelters-egui/tests/gallery.rs::utilities_page_generates_its_classes` (needs the gallery build).
- `packages/native-runtime/test/style.test.mjs`: the GTK warning fires once, never for semantic classes.

Run locally after the last code edit: `cargo nextest run -p svelters-egui --test utilities --test gallery`
(14 passed, with the gallery built by `pnpm build`), `cargo check -p svelters-egui -p svelters-style
--all-targets`, `cargo clippy -p svelters-egui --all-targets` (no warnings in the touched code),
`node --test test/style.test.mjs` in native-runtime (5 passed) and its `pnpm run check`. Not run locally:
the rest of svelters-egui's tests (other test binaries), GTK/Qt backends (only the JS runtime changed for
them), the rest of the workspace. CI is the gate.

## Next (not U3)

- U4: the compiler checker (diagnostics, `@apply`).
- U5: class-only HMR edits; U6: `group-*`/`peer-*` (E6), `@utility`; U7: measurement.
- Breakpoints from the active theme (needs `UtilityTokens` to carry values).
- A docs guide "Utility classes" (U8) should mention `EmbedConfig::with_utilities` and `--utilities`.
