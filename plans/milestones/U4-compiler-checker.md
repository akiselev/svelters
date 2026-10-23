# U4: the compiler's utility-class checker, `@apply` and fix-its

Lane U4 on the WSL PC, branch `ci/u4` from master 5aefc137. The design is
`plans/research/UTILITY-CSS.md` §4.1 (the compiler checks, the host generates), §4.2–§4.4, §4.6, §4.7 and
§7 (U4's row and acceptance tests), with the user's decisions in §9: only checking happens at compile time, utilities are opt-in
with an optional prefix, and the tokens are ours only. The checker uses U1's `svelters_style::Resolver` and the
generated vocabulary (`svelters_style::utilities`), so the compiler and the runtime agree on every class.

## What landed

| Item | What | Where |
|---|---|---|
| Option | `CompileOptions::utilities` and `CheckOptions::utilities: Option<UtilityCheck { prefix, strict_tokens, scan_script, tokens }>`. Off by default (UQ8). It runs only on a backend whose style engine runs in the backend (egui). `tokens` adds an app theme's `(name, type)` tokens to the built-in ones, so `bg-cad-panel` is a value. CLI: `--utilities`, `--utilities-prefix p`, `--utilities-strict`, `--utilities-scan-script`. Vite: `svelteNative({ utilities: true \| { prefix, strictTokens, scanScript } })`. | `utilities/mod.rs`, `compile.rs`, `check.rs` (also in the reuse key), `bin/svelters-compile.rs`, `packages/native-runtime/vite.js` |
| Checker | It checks every complete class token on a native element: the static `class="…"` text, `class:<name>` directives, and the string literals and template-literal text inside `class={…}` (arrays, object keys including identifier keys, `?:`, `&&`/`\|\|`/`??`, and `cn(…)`/`clsx(…)` calls). With `scan_script`, it also checks the string literals and template text in `<script>`, skipping imports, and reports only the resolver's verdict there. | `utilities/mod.rs` |
| Codes | 11 new CX1 rows (`svelters::utility::*`): `utility_unknown`, `utility_unsupported` (also used for `Pending`), **`utility_bad_value` (its own code, an error)**, `utility_arbitrary_color` (an error under `strict_tokens`), `utility_alias` (a note), `utility_variant_never_matches`, `utility_not_applicable`, `utility_conflict`, `utility_dynamic_value`, `utility_shadowed_class`, `utility_apply_unknown` (an error). The docs page is regenerated. | `codes.rs`, `docs/site/.../diagnostics.md` |
| Per kind | From `KINDS` and the egui table, on built-in tags: (1) A state the kind never sets warns (`checked:` on `<box>`); `focus-visible` counts as `focus`; a kind with an empty state list is not checked. (2) `part-<name>:` and `[&::part(x)]:` must name a part of the kind. `scrollbar-x/y`, `thumb-x/y`, `corner` and `row` are accepted on every kind (U0 note 3). The fix-it suggests a similar part. (3) A state after a paint part is checked against the host's states (DOC2). (4) `state-[sv.*]:` must be a provider state the kind sets, while app namespaces aren't checked. (5) `utility_not_applicable` fires when a property's `tags` exclude the kind, unless the rule styles a part or another element. | `Cx::per_kind` |
| Conflicts | Within one static list, two utilities whose property sets are equal and whose variants, media and `!` match conflict. The loser in canonical order (vocabulary row, then class name, as the cascade sorts them) is reported with a fix-it that removes it. `p-1 px-2` doesn't conflict, since the property sets differ and `px` wins by rank on purpose. | `Cx::conflicts` |
| Dynamic values | Text right before an interpolation that is a namespace (`p-{n}`, `w-[{px}px]`, `` `p-${n}` ``) warns, with help naming `style:<prop>={…}`. | `Cx::dynamic` |
| Shadowed | A class selector in `<style>` (Svelte's CSS AST) that resolves as a utility warns. | `shadowed` |
| `@apply` | Expanded before scoping on egui, **whether or not utilities are on** (it's explicit and needs no runtime). The embed's prefix applies when set. Output: declarations, nested `&:hover { … }` / `&::part(x) { … }` and `@media … { & { … } }`; `!` or a trailing `!important` adds `!important`. The rewritten source goes to the Q3w scoping (`scoped_css`) and to the CS5g engine check (`engine_css`), and their spans map back through `Expansion::to_orig`. The compiler's own native-style parser still reads the original, which ignores `@apply`. On the Vite path, egui sheets with `@apply` now go through `--css` (the Rust port) instead of stock Svelte. Unknown names are `utility_apply_unknown` errors with "did you mean". | `utilities/apply.rs`, `scoped_css.rs`, `lower/engine_css.rs`, `lower/mod.rs` |
| Fix-its | "Did you mean" for values (the resolver's suggestions) and variants (`closest` over the variant names). Unsupported names get their replacement: `ring-2` → `outline-2` keeps the value, `forced-colors:` → `contrast-more:`, `tw:block` → `tw:flex`. Aliases get the canonical spelling (`aria-checked:` → `checked:`, `[&::part(x)]:` → `part-x:`). Conflicts remove the loser, and unknown parts suggest a similar part. **Each fix-it is offered only if the rewritten class resolves as a utility.** They are offered only where the token text can be rewritten (attribute text and string literals, not identifier keys). | `Cx::fixes` |
| `class:…/` | `class:w-1/2={x}` fails in Svelte's parser (`expected_token` at the `/`). That parse error gets the fix-it `class={{ 'w-1/2': x }}`, unless the start tag has another `class` attribute. | `fixits.rs` `class_with_slash`, `codes.rs` (`expected_token` row's fix) |

**Semantic classes stay plain.** `NotUtility` is never reported. A bare name that egui doesn't support
(`block`, `fill`, `content`, `accent`) is not reported without a prefix, since it is as likely a semantic class. With a
prefix (`tw:block`), it is reported (the U1 note).

## Tests

`crates/svelters-compiler/tests/utilities.rs` has 14 tests:

- Each of the 11 codes fires on a minimal component.
- `assert_fixes_resolve` applies every fix-it and checks again. The code must drop, and the first fix-it must add nothing.
- `checked:` warns on `<box>` but not on `<checkbox>`.
- Parts, provider states and `not_applicable` are covered.
- clsx and `class:` tokens are checked.
- The script scan is opt-in.
- With the option off, there are no utility diagnostics.
- `@apply bg-surface hover:bg-list-hover dark:p-1 part-thumb:bg-accent!` scopes **byte-identically** to the hand-written CSS.
- Positions after an expansion map back.
- The egui fixtures (gallery-egui, embed-egui, counter-, motion-, lists-egui, examples/egui-kinds) produce no utility diagnostics, even with the script scan on.
- With `SVELTERS_CADATOMIC_UI=<cadatomic>/ui`, the test also checks cadatomic's panels. It was run locally against `~/projects/cadatomic/ui` (3 panels) and they are clean.

`utilities::apply` unit tests cover statement finding and span mapping.

Run locally, after the last code edit: the `utilities` and `cx1_codes` test binaries (17 tests, `UPDATE=1`
once and then again without it); `cs5g_egui_css`, `q3w_css_wiring`, `cx1_check` and `scoped_css::tests`, whose code paths changed (26
tests, all passing); `cargo check -p svelters-compiler --all-targets`; `cargo clippy -p svelters-compiler
--all-targets`, with no findings in touched code; and `node --check packages/native-runtime/vite.js`. Not run locally: the rest of
the compiler's tests, the workspace, the JS packages (the Vite plugin change is not exercised by a local
test) and conformance. CI is the gate.

## Handoff and next steps

- **The loader and the embed don't pass the option yet.** `svelters-modules`' live loader builds
  `CompileOptions` without `utilities`. It should set them from `EmbedConfig::utilities` (U3) and pass the
  theme's tokens as `UtilityCheck::tokens` (the same `(name, type)` pairs as `set_utility_tokens`). That is
  a one-line change once U3's config exists, and it belongs to U3 or a follow-up, not to U4.
- **App tokens at compile time.** Without `tokens`, an app colour such as `bg-cad-panel` is a plain class to the
  checker (`NotUtility`, so it isn't reported). `@apply bg-cad-panel` is an error, though, until the tokens are passed.
- **Node parts.** Once svelters-egui declares the scroll and row node parts (U0 note 3), drop
  `UNDECLARED_PARTS` and let node parts check their own states (`part-thumb:hover:`).
- `utility_variant_never_matches` skips kinds whose generated state list is empty (`tree-view`,
  `virtual-list`, `menu`…), because empty there may mean "not dumped" rather than "none".
- `@apply` at the top level of a sheet, outside a rule, is expanded as is, so the declarations end up at the top
  level and Svelte's CSS parser reports them.
