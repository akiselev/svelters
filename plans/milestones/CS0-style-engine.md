# CS0: the Rust CSS engine crate (`crates/svelters-style`)

Design: `plans/research/CSS.md` §2 (§2.1–2.5, §2.9), the §5 CS0 row and §8 D1–D10 as recommended (user,
2026-10-22); `plans/research/TRACKS.md` §2.1 STYLE, row CS0. The `Element` trait is shaped after ARCH-SCENE §6's
`NodeRef` and `StateBits` (SG1/SG2). Starts from master `f0a242d9`. Lane: STYLE (CS0), branch `ci/cs0`.

## What landed

| File | Content |
|---|---|
| `Cargo.toml` | `cssparser =0.38.0`, `selectors =0.41.0`, `cssparser-color =0.6.0`, pinned exactly (D5; `selectors` breaks every 0.x). No toolkit, no egui. One workspace member line. |
| `atom.rs` | `Atom` (`Arc<str>`, `ToCss` as an identifier, `PrecomputedHash`) and `AttrValue` (serialises as a string), the `selectors` string types. |
| `state.rs` | `StateMask(u64)` with **the scene's `StateBits` layout** (bits 0–13 the same built-ins, 32–63 provider states), `BUILTIN` names, `STATE_PSEUDO_CLASSES` (`:hover`→hovered, `:active`→pressed, `:focus`, `:focus-within`, `:disabled`/`:enabled`, `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`/`:valid`, `:read-only`/`:read-write`, `:dragging`, `:exiting`, `:busy`), and `StateNames`, which resolves provider states at parse time. |
| `selector.rs` | `Impl: SelectorImpl`, `PseudoClass` (`State`, `FocusVisible`, `Provider`), an uninhabited `PseudoElement`, and `SelectorParser`: `:is/:where/:not/:has`, `:nth-child(… of S)`, `&`, and provider states as **`:state(ns.name)`** (e.g. `:state(cad.pending)`). `parse_selector_list` for queries. |
| `element.rs` | **`Element`**: `Copy + Debug` handle with `key() -> NonZeroU64` (`NodeId::to_raw()`), `parent`, `prev_sibling`, `next_sibling`, `first_child`, `tag`, `id`, `has_class`, `for_each_class`, `attr(name) -> Option<Cow<str>>`, `state() -> StateMask`, and `is_focus_visible` (default: focused). An internal adapter implements `selectors::Element`. Not HTML: tags and attribute names match case-sensitively; no shadow DOM, slots or parts. Shadow and attached children aren't light children, so a provider's shadow root is `:root` of its own tree. |
| `sheet.rs` | Our own sheet parser on cssparser's tokenizer, with 1-based line/column on every rule and declaration: style rules, **nesting** (relative selectors and selectors without `&` get `& ` prepended, then `replace_parent_selector`), `@media` (top level and nested; inside a style rule its declarations apply to the parent), `@layer` (statement, block, anonymous, dotted, nested), `!important`, custom properties (case-sensitive names, raw values). Unknown at-rules (`@keyframes`, `@supports`, `@container`, `@import`, …) are dropped with a warning. Errors drop the rule or declaration, as CSS does, and become `Diagnostic`s. `DeclBlock::parse_inline` for `style=`. |
| `values.rs` | `ComputedValue` (`Length{px, percent}`, `Auto`, `Number`, `Int`, `Bool`, `String`, `Keyword`, `Color`, `Insets`, `FontWeight`, `Raw`) and the value parsers by the table's type. Lengths: the table's units, `em`, `rem`, `%`, and `calc()`/`min()`/`max()`/`clamp()` with `+ - * /`, nested parentheses, `pi` and `e`, all type-checked. Unitless numbers are lengths in the toolkit's unit, as in ST1's setter grammar. Colours: CSS Color 4 through `cssparser-color` (hex, named, rgb/hsl/hwb, lab/lch in D50, oklab/oklch, `color(srgb|srgb-linear)`, `currentcolor`), converted to sRGB. `bolder`/`lighter` from the CSS Fonts table. |
| `var.rs` | `var()` substitution over tokens (spaces around each substitution so tokens never paste), fallbacks, nesting, and a 64 KB output cap that stops doubling chains. |
| `media.rs` | `MediaList` and `MediaEnv`: `all`/`screen`/`print`, `not`/`only`/`and`/`or`, `width`/`height`/`aspect-ratio` with `min-`/`max-` and the range syntax (`400px <= width <= 50em`), `orientation`, `prefers-color-scheme`, `prefers-reduced-motion`, `prefers-contrast`. Unknown features and general-enclosed terms are false; a query that doesn't parse is `not all`. |
| `cascade.rs` | **`Stylist`** for one backend `Table`. `add_sheet(sheet, Origin)` checks every declaration against the table (unknown property: warning naming the backends that have it; bad value: error; an `inline_only` property in a rule: error). Also `remove_sheet`, `set_media(env) -> Vec<RuleId>` (the rules whose `@media` flipped), a **rule index** bucketed by the subject compound's id, then class, then tag, then universal (Stylo's `SelectorMap`), `matched_rules`, `cascaded` and `compute`. Cascade order: origin and importance (theme < author < inline; author `!important` < inline `!important` < theme `!important`), `@layer` (reversed for `!important`; a layer's own rules after its sublayers; unlayered last), specificity, source order. `matches` and `select` serve queries (CSS.md §2.9). `MatchCaches` wraps the `:nth-child`/`:has` caches. |
| `computed.rs` | **`ComputedStyle`**: values by table index, plus inherited custom properties. Customs resolve `var()` among themselves, and a cycle makes every member guaranteed-invalid. The CSS-inherited properties (color, font-*, letter-spacing, line-height, text-align, visibility, cursor, …) inherit. `inherit`/`initial`/`unset` are supported; `revert`/`revert-layer` act as `unset` (there is no user-agent sheet). A value that fails after substitution is unset. `font-size` is computed first (its `%`/`em` use the parent's size) and `color` second (for `currentcolor`). `changed_since(&before)` returns the property indexes a setter backend must push or restore. |
| `table.rs`, `tables.generated.rs` | `Table`, `Prop`, `Via`, `ValueType`, `Dialect`. The five toolkit tables are **generated by `tools/metadata/style/gen_props.py`** (now also writing the Rust file; its `--check` in `pnpm test:meta` covers both outputs). `Table::computes(prop)`: setters and child attributes always, `via: css` only when the dialect is `None` (egui), because then no toolkit engine decides the value (asked for by CS5e). |
| `invalidation.rs` | **`Invalidation`**, v2 of CSS.md §2.5 (dependency maps from the selectors, not per-node flags). For each class, id, attribute and state bit: a `Reach` of `SELF` (the subject compound), `DESCENDANTS` (an ancestor compound), `LATER_SIBLINGS` (left of `+`/`~`), `ANCESTORS` (inside `:has()` in the subject: ancestors and earlier siblings) or `TREE` (`:has()` elsewhere). Nested `:is/:where/:not/:nth-child(of)` lists take the enclosing position. `structure_changed()` covers `:nth-*`, `:empty`, sibling combinators and `:has`. `states_used()` is the tracking gate of §2.3, and `uses_attr()` the attribute gate of §2.2. `for_each_reached(node, reach, f)` walks the reached nodes. |

Also: `THIRD_PARTY_NOTICES.md` notes the MPL-2.0 dependencies (no source copied). `.github/workflows/linux-gtk.yml`
adds `-p svelters-style` to the portable nextest step.

## D1 (standard CSS names)

The engine takes property names from the tables, which are already standard CSS where the mapping is faithful
(egui: `flex-direction`, `gap`, `width`, `align-items`, …) and native where CSS can't say it (`hexpand`). An unknown
property is a warning naming the backends that have it. Keyword value maps (`row → horizontal`) are not in the
metadata yet, so `Enum` values are the table's own keywords.

## Tests (`cargo nextest run -p svelters-style`)

- `tests/selectors.rs`: a WPT-style corpus of 46 selectors over one tree, covering type/class/id/universal, all seven attribute operators and `i`, the four combinators, `:is/:where/:not/:has` (including `:has(+ x)`), `:root`, `:empty`, `:first/last/only-child`, `:nth-child(An+B [of S])`, `:nth-of-type`, every state pseudo-class, `:focus-visible`, `:state()`, and Svelte's scoped shape. Also rejections, and the three specificities from the CSS.md §2.1 probe.
- `tests/cascade.rs`: specificity and source order, origins and `!important`, inline blocks and inline-only properties, layers (normal, nested, and reversed for `!important`), inheritance and CSS-wide keywords, `em`/`rem`/`%` font sizes and `calc(100% - 1em)`, custom properties (override, inheritance, cycles, fallbacks, invalid at computed time), `via: css` exclusion and the `Dialect::None` rule, table diagnostics on the egui table, media switching with flipped rule ids, matched-rule order and sheet removal, `changed_since`, and the egui table end to end.
- `tests/invalidation.rs`: the reach by position for classes, states, attributes, ids and structure; the walks; and a soundness check: for each class change, every node whose matched rules change lies inside the reported reach.
- `tests/fuzz.rs`: 3000 deterministic mutated or random sheets through parse, `add_sheet`, inline parse, compute over a tree, selector parse and media parse. Nothing may panic.
- Unit tests in `values.rs` (lengths, `calc`, numbers, keywords, insets, font weights, colours), `media.rs`, `var.rs`, `sheet.rs` (nesting, layers, recovery, provider states, inline) and `table.rs` (the generated tables are sorted and typed).

## Not done (follow-ups)

- **Bloom filter.** `add_element_unique_hashes` returns false and matching passes no filter. The rule index does the coarse rejection. Add the ancestor bloom when CS1/CS5e measure on the 10k-row list.
- **Invalidation v1 (selector flags) is not used.** v2 maps only. `:has()` outside the subject reaches the whole tree, the conservative start CSS.md §6 suggests. Measure in CS5e.
- **Pseudo-elements** (`::placeholder` …), `:placeholder-shown`, `@container`, `@supports`, `@keyframes`/transitions (CS6), `@font-face` and `@import`.
- **`min()`/`max()` mixing `%` with px** is an error, because they can't be compared before layout. taffy's calc support (CS5e) can lift this.
- **Lossless spans.** The sheet keeps line/column, not byte ranges. Q3 needs its own span-preserving AST for MagicString-style rewriting.
- `revert`/`revert-layer` behave as `unset`.
- Explicit-prop ownership (CSS.md §2.8) and the push to backends belong to CS1/CS5e. `ComputedStyle::changed_since` is the hook.

## Not run locally (CLAUDE.md: CI is the gate)

Run locally: `cargo check -p svelters-style --all-targets`, `cargo clippy -p svelters-style --all-targets` (clean),
`cargo nextest run -p svelters-style` (all pass) and `python3 tools/metadata/style/gen_props.py --check`. Not run:
the workspace nextest, `pnpm test:meta` and the other gates, conformance (no compiler or runtime change), and the
other backends' suites.
