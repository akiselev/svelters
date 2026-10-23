# U1: utility-class resolver and utility layer (E1, E2, E4)

Lane U1 on the WSL PC, branch `ci/u1` from master 943b4c67. **U2 (`ci/u2`) is merged into this
branch** (merge 28880037, decided by the coordinator, 2026-10-22: U2 and U1 land together). The design is
`plans/research/UTILITY-CSS.md` §4.1–§4.5, §5 (E1, E2, E4) and §7 (U1's row and acceptance tests), with
the user's decisions in §9 taking precedence: our tokens only, utilities below component CSS, opt-in with
an optional prefix, typed arbitrary values, and `part-<name>:`.

## U0 + U2 merge fixes (in 28880037)

- `lib.rs` re-exports both `MEDIA_FEATURES` (U0) and `Platform` (U2).
- `MEDIA_FEATURES` and `egui.json` `style.media_features` list `-sv-platform` (`macos`, `windows`,
  `linux`), and `media_features_table_matches_the_evaluator` varies the platform. The egui.json entry
  was written by hand in the dump format, and CI's `svelters-egui --test spec` confirms it.
- `gen_utilities.py`: duration and integer sources fit `time-list`, and easing fits `easing-list`
  (U2's longhand types). After regeneration, `duration-*`, `delay-*`, `ease-*` and `mac:`/`win:`/`linux:`
  are no longer pending (U0 note 1).

## What landed

| Item | What | Where |
|---|---|---|
| **E1** resolver | `Resolver { table, tokens, prefix, strict_tokens }.resolve(class) -> Resolved` (`NotUtility`, `Utility(Utility)`, `Invalid(UtilityIssue)`). It implements the §4.2 grammar: the prefix as the first variant; variants stacked left to right; `!` important, trailing (or leading); a `-` negative (only on rows with `negative: true`, emitted as `calc(v * -1)`); `[prop:value]` arbitrary properties (table properties, shorthands or `--custom`); and values from keywords, token scales, `fraction`, `int-px`, `number`, `percent`, `integer` templates, `[arbitrary]` (`_` is a space and `\_` an underscore; Tailwind type hints `color:`, `length:`, `number:`, `percentage:`, `shadow:`, `family-name:`) and `(--var)`. Modifiers: `/NN`, `/[0.25]` and `/(--x)` on colours (`color-mix(in oklab, c NN%, transparent)`), and `/6` on font sizes (`line-height` from the space scale). Rows sharing a name (`text-`, `border-`, `font-`) are tried longest name first, then by rank. An untyped `[...]` tries typed properties before raw ones, so `font-[600]` is a weight and `font-['Inter']` a family. | `svelters-style/src/utility.rs` |
| E1 variants | States (`STATE_VARIANTS`, `focus-visible` included), aliases (`aria-checked:`, with an `Alias` note), `placeholder:`/`selection:`, structural (`first:`…), `part-<name>:` → `::part(name)`, `state-[ns.name]:`, `prop-<name>:` → `[name="true"]` and `prop-[name=value]:`, compounds `not-`/`has-`/`in-` over a state or `[selector]`, media (`dark:`, `contrast-more:`, `motion-reduce:`, `mac:`…), breakpoints `sm:`…`xl:` and `max-*:` (the built-in theme's px), and arbitrary `[&>row]:`. `group-*`/`peer-*` (E6) and `@sm:` (CS-CQ) resolve to `Pending`; `UNSUPPORTED_VARIANTS` resolve to `Unsupported` (longest name wins). | `utility.rs` |
| E1 tokens | `UtilityTokens`: token name → type (`length`, `color`, …), without the `--sv-` prefix. `UtilityTokens::builtin()` is the generator's defaults; a backend builds its own from the active theme (`FromIterator<(name, type)>`, `insert`). Scale patterns (`space-*`, `*-h`, exact tokens, excludes) resolve against it at run time, so `bg-cad-panel` works once the theme has `--sv-cad-panel` of type `color`. | `utility.rs` |
| E1 issues | `UtilityIssue { class, kind, severity, message, suggestions }`. `IssueKind` is `#[non_exhaustive]`: `UnknownValue`, `UnknownVariant`, `Unsupported { replacement }`, `Pending { requires }`, `BadValue`, `ArbitraryColor`, `Alias { canonical }`. `kind.code()` gives the CX1 code U4 should register, and `is_note()` marks lints that still generate the rule. `UnknownValue` carries "did you mean" suggestions (Levenshtein ≤ 1 or 2 against keywords and the built-in scale names: `bg-acent` → `bg-accent`). | `utility.rs` |
| **E2** utility layer | `Stylist` gains utilities. `ensure_utilities(&[Atom], &ParseOptions) -> Vec<UtilityIssue>` resolves each class it hasn't seen, generates the CSS text (`@layer utilities { [@media …{] sel { … } }`), parses it with `Stylesheet::parse`, checks it with the table, and **appends** the rules to the index and the invalidation maps without `rebuild()` (`Stylist::append_entry`). The one exception: when the `utilities` layer isn't declared yet, the first batch rebuilds once, because layer ranks change. Known classes and a negative set (non-utilities and rejected classes, capped at 16k) make a second sight one hash lookup, and issues are reported once. **Canonical order**: `Entry::order` is now `u64`. Sheet rules keep their sequence number; utility rules get `1<<62 | variant_rank<<16 | row_rank`, and the class name breaks ties (the sort keys gained a `&str`). The cascade therefore doesn't depend on discovery order. **GC**: `retain_utilities`/`release_utilities` refcount classes through node class lists, and `collect_utilities()` drops unreferenced rules oldest first down to `high_water` (default 4096). It marks entries dead (skipped by matching), and the next rebuild compacts them; one runs when dead entries pass half. Config: `set_utilities(UtilityConfig { enabled, prefix, strict_tokens, high_water })`, off by default (UQ8). Turning utilities off or changing the prefix drops every generated rule. `set_utility_tokens` retries rejected classes. Stats: `utility_stats()` (generated, live, negative, dropped, batches) and `rebuild_count()`. | `utility.rs` (layer), `cascade.rs` (`Entry`, `append_entry`, `rebuild` compaction and counter, sort keys, `check_decl` now `pub(crate)`) |
| **E4** `color-mix()` | `color-mix(in srgb \| srgb-linear \| oklab, c1 [p1], c2 [p2])` per CSS Color 5 §2: percentages normalised, a sum below 100% scales alpha, premultiplied interpolation, with sRGB↔OKLab both ways. Parsed wherever a colour is (gradients included), after `var()` substitution. | `values.rs` (`color_mix`, `srgb_to_oklab`, `linear`) |

## API for U3 (egui wiring)

- At embed setup: `stylist.set_utilities(UtilityConfig { enabled, prefix, strict_tokens, ..Default::default() })`
  from `EmbedConfig::utilities`.
- `stylist.set_utility_tokens(tokens)` on every theme change: build it from the theme's token
  manifest as `(name, type)` pairs, using the same types as `egui.json` `theme.tokens[].type`.
- The theme's `@layer` statement should declare `components, utilities` before any utility is
  generated. Otherwise the first batch rebuilds once (harmless, but it costs one rebuild).
- In `apply_classes`, **before** computing the class-change reach:
  1. call `ensure_utilities(&new_classes, &opts)`, passing the scene's `StateNames` in `opts`
     for `state-[…]:`, and log the returned issues once in dev;
  2. call `retain_utilities(added)` and `release_utilities(removed)` (and release a removed
     node's classes);
  3. call `collect_utilities()` once per restyle pass.

  Batch the misses of one pass into one `ensure_utilities` call where convenient. The issues
  are per class and reported once, so dev logging can print them directly.
- `invalidation().states_used()` grows when a state variant is generated (`checked:bg-x`). If the
  scene caches the used states, refresh the cache after an `ensure_utilities` call that generated
  something (`utility_stats().generated` changed).
- Breakpoint variants use the built-in theme's px (`BREAKPOINTS.sizes`; U0 note 8). Re-resolving
  them from the active theme would need `UtilityTokens` to carry values; it doesn't yet.
- GTK/Qt: these backends don't call `ensure_utilities`. The warning is U3's.

## For U4 (compiler checker)

- Use `Resolver::new(&tables::EGUI, &UtilityTokens::builtin())` (plus `prefix`/`strict_tokens`)
  per class token. `Resolved::Invalid(i)` maps to `i.kind.code()`; `Utility(u).notes` holds the
  lints (`ArbitraryColor` is an error under `strict_tokens`, and `Alias` is info).
  `i.suggestions` are the fix-its for `utility_unknown`. `BadValue` reports as
  `utility_bad_value`, a code §4.7 doesn't list: register it, or fold it into `utility_unknown`.
- **Semantic classes stay plain**: a class that starts with a namespace but whose value isn't
  shaped like one (not a digit, `[`, `(`, or a near miss of a name) is `NotUtility`. So
  `my-widget`, `top-bar`, `text-input` and `left-pane` are never reported, while `bg-acent`, `p-x`
  and `-p-2` are.
- **`bg-editor` is a plain class.** The acceptance test "bg-editor resolves to var(--sv-surface)"
  predates §9: the user chose our role names, and U0 doesn't emit VS Code aliases (U0 note 7). The
  U1 test asserts `NotUtility`. U4 can suggest the role from `theme.tokens[].vscode`.
- An unsupported name that is also a plausible semantic class (`content`, `list`, `fill`,
  `snap`) resolves to `Unsupported` when used bare. At run time that is one dev warning per
  class. The compiler may want to downgrade bare names.

## Notes and choices

- **U0 note 4 (bare `border`)**: on egui a width alone draws. `StyleView::border_side` takes
  `solid` when no style is set, so `border` stays width-only.
- `hover:` vs `focus:` on one property with equal specificity and rank: the class name decides
  (`hover:p-2` beats `focus:p-3`), not Tailwind's variant order. It is deterministic, and
  `utility_conflict` (U4) flags it.
- Generated CSS is safe from dynamic strings: classes with whitespace, control characters, `{`,
  `}` or `;` are never utilities. The selector is the class escaped with `serialize_identifier`,
  and a value that breaks parsing is rejected as `BadValue`.
- `LANDED` (`utility.rs`) lists the engine items that are in: E3, E4, E5, E7. U6 adds E6 there (and
  CS-CQ when it lands), which switches on `group-*`/`peer-*`/`@sm:`.

## Tests

- `svelters-style/tests/utilities.rs` (new, 12 tests; the §7 U1 acceptance list):
  - `every_vocabulary_row_resolves_and_parses` is U0's planned check: every non-pending row,
    for each keyword, each built-in scale value and one sample per open source (over 1000
    utilities). Each resolves through its own row, and its CSS parses and passes the egui table
    with no diagnostics, both as generated and with the built-in theme's token values substituted.
  - `variant_stacking_order` covers `hover:part-thumb:` vs `part-thumb:hover:` (selector text
    and matching) and `dark:hover:`.
  - `colours_are_theme_roles`: `bg-surface` is `var(--sv-surface)`, `bg-editor` is plain, and an
    app token works.
  - `opacity_modifier_mixes_the_colour`: `/50` → rgba(0,120,212,128).
  - `cascade_position_and_important`: above the theme, below a component rule, `!` beats it.
  - `arbitrary_values_are_type_checked`, including the colour lint and strict mode.
  - `order_independence`: two discovery orders give identical `cascaded()` lists.
  - `appending_never_rebuilds`: 1000 batches, `rebuild_count` unchanged.
  - `invalidation_knows_new_utilities`, `negative_cache`, `gc_drops_only_unreferenced_rules`,
    `opt_in_and_prefix`, `part_utilities_use_the_part_index`.
- `utility.rs` unit tests: `lexical_helpers`, `values`, `issues`, `variants_stack_left_to_right`, `prefix`.
- `values.rs::tests::color_mix`.

Run locally after the last code edit: `cargo nextest run -p svelters-style` (85 passed), `cargo clippy -p
svelters-style --all-targets` (clean), `cargo check -p svelters-style --all-targets`,
`gen_utilities.py --check`, `gen_reference.py --check`, and the `tools/metadata/style` pytest (27).
Not run locally: `svelters-egui` (no code change there beyond U2's, whose own runs are recorded in
U2's milestone; its `--test spec` checks the hand-written egui.json entry in CI), the JS packages,
and the rest of the workspace. CI is the gate.
