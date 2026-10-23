# U0: utility-class metadata and generator

Lane U0 (WSL PC), branch `ci/u0` from master 30a0be2c, merged with master c0faf05a. Design:
`plans/research/UTILITY-CSS.md` §3 (option G), §3.4 (missing metadata), §7 (lanes) and §9 (the user's
decisions of 2026-10-22, recorded there by the coordinator).

**Followed the user's decisions over the doc's recommendations (UQ4/UQ5): our tokens only.** Tailwind's
property names, our values: `p-7` is `var(--sv-space-7)` (32px), `text-label1` is `--sv-font-size-label1`,
colours are theme roles and component colours. No 4px multiplier (`--sv-spacing` is not added), no
Tailwind type scale, no palette.

## What landed

### Metadata dumped by `spec.rs` into `metadata/semantics/egui.json`

| Key | Content | Source |
|---|---|---|
| `kinds[].parts` | `{name, node, doc}` per `#[part]`, in declaration order; `[]` for kinds without an own widget | `KindRegistry::replacement(i).entry.parts` (built-ins), `KindEntry.parts` (fragments; only when non-empty, so existing fragments don't change) |
| `kinds[].provider_states` | the `sv.*` states a kind sets (`sv.overflow-*` on scroll-area, scrollbar, virtual-list, tree-view) | `kinds::docs::provider_states` (new) |
| `style.provider_states` | every `sv` provider state `{name, namespace, state}` | `provider::state_registry()` |
| `style.media_features` | `{name, type: range/discrete, values}` | `svelters_style::MEDIA_FEATURES` (new, tested against the evaluator) |
| `style.properties[].range` | `{min?, max?}` | `style::PROP_RANGES` (new): non-negative sizes, paddings, gaps, radii, widths, font-size; opacity 0..1; margins and insets unbounded |
| `theme` | `sets`, `densities`, `prefix`, `tokens`: per `--sv-*` token its `layer` (role, component, density, shape, set), `type` (color, length, shadow, duration, easing, font-family), `vscode` key (roles), `from` (components), `value` or `values` per set, `density` values | `theme::token_manifest()` (new), reading `ROLES`, `COMPONENT_*` and `theme/data/*.json` |

`theme/data/shape.json` gained `space-0` (0px), `breakpoint-sm/md/lg/xl` (640, 960, 1280, 1600 px) and
`container-xs/sm/md/lg/xl` (240, 320, 480, 640, 800 px; the doc's proposal).

### Ranges in the style tables

- `metadata/style/*.yaml` rows take an optional `range: {min, max}` (setter, child-attr and decl rows of a
  numeric type; `style_table.py` validates it). None of the four toolkit YAMLs sets one yet.
- `gen_props.py` carries `range` into `props.generated.js` and `Table::ranges` in `tables.generated.rs`
  (sorted `(name, min, max)`, infinite open ends) with `Table::range(name)`. The `Table` struct gained the
  field; the two test tables in svelters-style were updated.

### Vocabulary and generator

- `metadata/style/utilities.yaml`: `config` (layer `utilities`, opt-in, optional prefix, colour-literal lint
  warning/strict error), `scales` (token families by pattern: space, font-size, family, radius, stroke,
  shadow, height, width, size, indent, motion, easing, breakpoint, container, colors), 122 `utilities` rows
  in canonical order, and `variants` (aliases, structural, pseudo-elements, compound, media, breakpoints,
  containers, the `part-`/`state-`/`prop-` prefixes, `focus-visible` as an extra state).
- `metadata/style/tailwind-v4.yaml`: every Tailwind v4 utility namespace and variant, plain (ours) or with
  a `reason` and `use` replacement (feeds `utility_unsupported`, U4).
- `tools/metadata/style/gen_utilities.py` (`--check`, `--report` for the to-do list) writes:
  - `crates/svelters-style/src/utilities.generated.rs` (`svelters_style::utilities`; types in
    `svelters_style::utility_table`): `CONFIG`, `SCALES` (patterns for run-time resolution plus the default
    theme's values), `ROWS`, `STATE_VARIANTS`, `ALIASES`, `PSEUDO_ELEMENTS`, `STRUCTURAL`, `COMPOUND`,
    `MEDIA`, `BREAKPOINTS`, `CONTAINERS`, the prefixes, `PROVIDER_STATES`, `KINDS` (per tag: states, parts,
    provider states), `UNSUPPORTED_UTILITIES`, `UNSUPPORTED_VARIANTS`;
  - `packages/style/src/utilities.generated.json` and `.js` (the manifest; `@svelters/style/utilities`);
  - `packages/style/src/utilities.completions.generated.json` (1700 concrete utilities of the default
    theme with their declarations and resolved px, open sources by sample, and the variants);
  - `docs/egui/reference/utilities.md` (fully generated: families, scales, colours per set, variants,
    parts and provider states per element, unsupported names).
- Checks (§3.6, each a failure): row properties in the egui table (or `requires:` = pending), value
  sources fit property types, keywords are enum values or literals of the type, `var(--sv-*)` names a
  token, no two rows generate one name, no concrete name under another row's number pattern, Tailwind
  list complete both ways (`own: true` for ours-only rows), no utility of ours under an unsupported
  prefix (except an entry's `allow`), unique variant names, aliases name states, media features known.
- CI: a `utility classes --check` step in `js.yml` after the widget reference check. The pytest file also
  runs under `pnpm test:meta` (`python3 -m pytest -q tools/metadata`).

### Tests

- `tools/metadata/style/test_gen_utilities.py` (14): a fixture proves each check fires (missing property,
  pending row, non-enum keyword, name collision, number-pattern clash, unlisted Tailwind namespace, a row
  missing from the Tailwind list, unsupported prefix over ours and `allow`, type misfit, unknown token,
  bad alias, unknown media feature and pending media, empty scale pattern); the checked-in outputs are
  current; the real vocabulary resolves to our tokens (`p-7`, `text-body`, `rounded`, `bg-list-hover`,
  `h-row`, `shadow-popup`, no palette, `p-0-5` not `p-0.5`).
- `tools/metadata/style/test_style_table.py`: `range:` accepted on numeric rows, rejected otherwise.
- `crates/svelters-egui/tests/spec.rs`: `kind_table_is_consistent` now covers parts (every own widget's
  `PartEntry`s, order and node/paint); `provider_states_are_listed_and_claimed`;
  `style_metadata_for_utilities` (media features, ranges, token manifest types/layers/sources).
- svelters-style: `media_features_table_matches_the_evaluator`, `generated_ranges_are_sorted_numeric_props`,
  `utility_table::tests::rows_agree_with_the_egui_table`.
- `packages/style/test/utilities.test.mjs`: module equals JSON; values are tokens.

Run locally (after the last code edit and the master merge): the three svelters-style tests above, the six
`svelters-egui --test spec` tests, all of `tools/metadata/style` pytest (27), `gen_utilities.py --check`,
`gen_props.py`, `gen_semantics.py --check`, `coverage.py --check`, `gen_reference.py --check`, and the
style package's `utilities.test.mjs` and `docs.test.mjs`; `cargo clippy -p svelters-style -p svelters-egui
--all-targets` (no new warnings in the touched code). Not run locally: svelters-egui's lib tests (theme
tests: `shape.json` only gained tokens), `egui-kinds-example` (its fragment has no parts, so it is
unchanged), the rest of the workspace. CI is the gate.

## For U1, U3 and U4 (metadata gaps and choices to know)

1. **Pending rows**: `duration-*`, `delay-*`, `ease-*` wait for E5 (the transition longhands are not in the
   egui table); `mac:`/`win:`/`linux:` for E7 (`-sv-platform`); `group-*`/`peer-*` for E6; `@xs:`..`@xl:`
   for CS-CQ. They are in the tables with `requires`/`pending`; a row activates by itself once its
   properties are in the table (regenerate).
2. **`focus-visible`** is not in `STATE_PSEUDO_CLASSES` (selector.rs parses it separately), so
   `style.pseudo_classes` and every `kinds[].states` lack it. The vocabulary lists it under
   `variants.extra_states`; per-kind validation (U4) should treat it like `focus`.
3. **No node parts yet**: every declared part is a paint part. The scroll bars (`scrollbar-x/y`,
   `thumb-x/y`, `corner`) and the lists' `::part(row)` are styled by name but not declared `#[part]`, so
   `part-thumb-y:`/`part-row:` aren't in `kinds[].parts`. Declare them (svelters-egui) or let U4 accept
   undeclared part names on those kinds.
4. **Bare `border`** sets only `border-width: var(--sv-stroke)`. Tailwind relies on preflight's
   `border-style: solid`; U1/U3 should check that a width alone draws, or add the style to the row.
5. **Font weight**: no weight tokens exist, so only `font-normal`/`font-bold` (plus `font-[600]`).
   `font-medium`/`font-semibold` are unknown (not unsupported). Adding `font-weight-*` tokens to
   `shape.json` and a scale would give `font-<name>`.
6. **Names chosen here** (the coordinator may change them in `utilities.yaml`): the bare font size is
   `text-body` (VS Code's `bodyFontSize`); spacing half steps are `p-0-5`/`p-1-5` (token names), not
   `p-0.5`; shadows are `shadow-0`..`shadow-4` (`elev-N`), `shadow-popup`, `shadow-window`, `shadow-none`;
   radius is `rounded` (bare), `rounded-xs/md/lg/xl`, `rounded-none`, `rounded-full` (no `rounded-sm`);
   families are `font-family`, `font-mono`; motion `duration-fast`, `duration-normal`; easing `ease-out`
   (our token) and `ease-linear`.
7. **Colours**: every `type: color` token, roles and component colours (`bg-button-bg-hover`). VS Code
   key aliases (`bg-editor`) are not utility names (the user: colours are our roles); `theme.tokens[].vscode`
   is there for U4's "did you mean". Two role names sit under unsupported Tailwind prefixes and are allowed
   explicitly: `text-shadow-color`, `bg-scroll-shadow`.
8. **Breakpoints and containers** are px of the built-in theme at generation (media queries can't use
   `var()`); U3 may re-resolve them from the active theme.
9. **Not named by any row** (reachable as `[prop:value]`): `background-image`, `content`, `visible`.
10. **Literal checks are heuristic** in Python; U1's test that every row parses through `Stylesheet::parse`
    is the real check (§7 U1).
11. **Not done**: the per-widget "Utilities" region in `gen_reference.py` (the reference page lists parts and
    provider states per element instead); the IntelliSense shim (U8).
