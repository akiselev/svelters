# UTILITY-CSS: Tailwind-style utility classes for svelters (egui first)

Status: research, 2026-10-22. Not a plan of record. Branch `research/utility-css`. Docs only: nothing was
built or run.

The request (user, through svelters-agent): research and design **a Tailwind-style utility class system** for
svelters. Later direction (user): **"Maybe we can autogenerate it from the metadata."** §3 takes that as the
central option.

Inputs read:

- `CLAUDE.md`, `STATUS.md` (OW, CS0/CS5e-g, DS1, LD2, Q4/Q4b, ZM1, DOC1/DOC2 rows);
- `plans/research/CSS.md` (D1–D10), `ARCH-WIDGETS.md` (parts, state bits, W3, Q3), `DESIGN.md` (R-rules,
  token layers L1–L3, densities), `WIDGET-PARITY.md` (P1–P10, CS-CQ);
- the milestones `OW-CSS.md`, `LD2-live-styles.md`, `Q4b-bundle-followups.md`, `H1-component-hmr.md`;
- `crates/svelters-style` (cascade, rule index, invalidation, values, media, tables, parse-cost test);
- `crates/svelters-compiler` (`scoped_css.rs`, `lower/engine_css.rs`, `codes.rs`, `fixits.rs`, the
  attribute parser in `svelte/parse/state/element.rs`);
- `crates/svelters-egui` (`style.rs` `apply_classes`, `theme/` with its `base.css` and token data,
  `provider/mod.rs` `PartEntry`, `spec.rs`);
- `packages/style`, `packages/native-runtime/src/style.js` (`applyEngine`), `types/egui.d.ts`;
- the metadata pipeline: `metadata/style/*.yaml`, `metadata/semantics/egui.json`,
  `tools/metadata/style/gen_props.py`;
- `docs/egui/reference/widgets/*.md` (DOC1) and `docs/egui/css/own-widgets.md` (DOC2);
- the web (§2), with a URL on every claim. Claims I could not confirm are marked **[unconfirmed]**, and
  claims from memory are marked **[unverified]**.

Repo facts come from master `c2931472`.

---

## 0. Summary

**Recommendation: Tailwind v4 class names, generated from svelters' own metadata plus a small hand-written
vocabulary, and resolved on demand by the Rust style engine in the host.** It is egui first, and it is opt-in per
embed.

1. **Generate the utility set from metadata (§3).** These sources already hold most of what a utility set needs:
   - the style tables (86 egui properties, each with a type, enum keywords and the kinds it applies to);
   - the 27 state pseudo-classes and each kind's states;
   - each kind's `#[part]`s;
   - DS1's theme data.

   What they lack is the **naming**. Tailwind's names (`p`, `px`, `items-center`, `text-` overloaded three ways)
   can't be derived. So:
   - a ~90-row hand-written vocabulary (`metadata/style/utilities.yaml`) names the Tailwind namespaces and their
     value sources;
   - a generator (`tools/metadata/style/gen_utilities.py`, `--check` in CI like `gen_props.py`) joins it with
     the tables, states, parts and tokens;
   - the generator emits the engine's utility table, a JS/JSON manifest, the docs pages and editor
     completions.

   Every keyword list, kind restriction, state variant, part variant and colour name comes from metadata. A new
   property, part or token appears as a utility without anyone editing a list.
2. **Generate rules at run time, in Rust, on demand (§4.1).** Every class string already reaches the host's
   `apply_classes` (egui, CS5e).
   - When a class the stylist has never seen parses as a utility, the engine writes its rule into an author layer
     `utilities`, once per process.
   - The same path serves Vite builds, the live loader, release `.svb` bundles (which carry nothing extra),
     extensions, dynamic strings such as `class="p-{n}"`, and cadatomic's Rust chrome (OW-RUST `imm`).
   - Twind's costs don't apply here (JS parsing, CSSOM insertion, a JS engine in the bundle): the engine is
     native and already resident.
   - The compiler scans templates only for **diagnostics** (unknown or unsupported utilities with fix-its, states
     and parts the kind never has, conflicts), never for generation.
3. **Compatibility (§4.3).** Real Tailwind v4 names and semantics where egui can render them: spacing, sizing,
   flex, grid, colours, typography, borders, radius, outline, opacity, shadow, cursor, overflow, transitions. Two
   exceptions:
   - values come from **our theme**, not Tailwind's;
   - unsupported Tailwind namespaces (transforms, filters, `ring`, `z-*`, `block`, ...) are a generated list with
     a reason and a replacement, reported as compiler diagnostics.

   We don't read a Tailwind `@theme` block; we *emit* one, for editor completions.
4. **Variants (§4.4).** These come from metadata:
   - every state pseudo-class (`hover:`, `focus-visible:`, `checked:`, `expanded:`, `current:`,
     `drop-target:`, ...);
   - `group-*`, `peer-*`, `has-*`, `not-*`, `in-*`;
   - structural variants (`first:`, `odd:`, ...);
   - media (`dark:`, `contrast-more:`, `motion-reduce:`).

   New:
   - **part variants `part-<name>:`** (left to right: `hover:part-thumb:bg-accent` is
     `.c:hover::part(thumb)`, and `part-thumb:hover:` is `.c::part(thumb):hover`), with `placeholder:` and
     `selection:` as Tailwind-compatible aliases;
   - platform variants `mac:`/`win:`/`linux:` (a new media feature);
   - `state-[ns.name]:` for provider states;
   - `prop-[name=value]:` for prop attribute selectors;
   - container variants `@sm:` once CS-CQ lands.
5. **Theming (§4.5).** Colour utilities read the DS1 roles (`bg-surface`, `text-fg`, `border-control-border`,
   `bg-list-hover`). They also accept the VS Code key each role is taken from (`bg-editor` resolves to role
   `surface`, `text-foreground` to `fg`), derived from the role table.
   - Everything resolves to `var(--sv-*)`, so it follows VS Code themes, high contrast, the affordance layer and
     cadatomic's overrides.
   - There is no Tailwind palette.
   - Spacing is Tailwind's 4px multiplier (`p-4` = 16px, any integer).
   - Radius names match VS Code's corner radii, which match Tailwind v4's.
   - `text-*` sizes are the theme's font-size tokens, so `text-base` is 13px, not 16px.
   - Arbitrary colours are a lint (warning, or an error under a strict profile). Arbitrary lengths are checked
     against the property's type.
6. **Integration (§4.6).**
   - Utilities sit in an author layer below unlayered component `<style>` rules, as Tailwind v4 does.
   - `class:` works except for names with `/`, which gets a fix-it.
   - `@apply` is expanded by the compiler before scoping.
   - `@utility` and `@custom-variant` are allowed in theme sheets, later.
   - **HMR:** a class-only edit to a static `class` attribute updates the live nodes without a remount. In dev, the
     compiler moves static class strings into a self-accepting `svelters-classes:` module, as LD2 did for
     sheets.
7. **Engine work (§5).**
   - Needed: an incremental utility layer (today every `add_sheet` rebuilds the whole index); a canonical rule
     order for on-demand rules; part-rule bucketing (today every `compute_pseudo` scans all `::part` rules);
     `color-mix()` for `/50` opacity; `transition-*` longhands; a platform media feature.
   - Measured first: an atomic-rule fast path and a style-sharing cache.
   - Later, with `group-*`: state invalidation qualified by the compound's class.
   - About 7–8 lane-days of engine work, plus up to 3 measured-first, in a total of about **21–25 lane-days**
     across 9 lanes (§7).

Open questions with recommendations are in §9.

---

## 1. What svelters has today (facts from the code)

### 1.1 The class path on egui

```text
Svelte template: class="…" / class={{…}} (clsx) / class:x={c}
  → @svelters/native-runtime applyEngine(el): classList(value).join(' '), diffed against el.sc.classes
  → host.style(OP.CLASSES, h, "a b c")                                   (one string per change)
  → svelters-egui style.rs: SVL_STYLE_CLASSES → apply_classes(id, Vec<Atom>)
       old ⊕ new classes → stylist.invalidation().class_changed(c) → Reach → mark_reach
  → restyle at the logic step (or same frame for state changes): Stylist::compute(node, …)
       matched_rules: candidates from the id/class/tag buckets (+ style_as, universal), sort, dedup
       → match_candidates (selectors::matches_selector) → order_decls (origin, layer, specificity, order)
       → compute(): parse each winning declaration's text against the table (var() substituted first)
```

- Every class string reaches Rust as text, whatever produced it (static, clsx, `class:`, a dynamic string). This
  is the hook for on-demand generation (§4.1).
- The runtime already has Svelte's `clsx` (`packages/native-runtime/src/index.js`), and `egui.d.ts` types
  `class?: ClassValue` and `` [directive: `class:${string}`] ``.

### 1.2 Engine facts that matter for atomic CSS

Each fact comes from `crates/svelters-style/src`.

| # | Fact | Where | Consequence for utilities |
|---|---|---|---|
| F1 | The rule index buckets each selector by its subject compound's id, else **first class**, else tag (Stylo's `SelectorMap`) | `cascade.rs` `RuleIndex::insert` | A utility `.p-4` sits alone in its own class bucket. A node tries exactly one candidate per utility class it carries: the best case for this index. |
| F2 | **Selectors ending in a pseudo-element go into one unbucketed list**. `matched_pseudo_rules_at` clones and tries all of them, and `has_pseudo_rules` scans them linearly | `cascade.rs` `index.pseudo` | Part variants (`part-thumb:bg-x`) would make every part computation on every node scan every part utility. **Needs bucketing** (§5 E3). |
| F3 | `add_sheet`/`remove_sheet` call `rebuild()`, which re-ranks layers and rebuilds the whole rule index and invalidation maps | `cascade.rs` `rebuild` | Adding utilities one rule at a time costs O(all rules) per add. **Needs an incremental append** (§5 E2). |
| F4 | Cascade sort key is `(origin, layer_rank, specificity, order)`, and `order` is the global registration sequence | `cascade.rs` `match_candidates`, `order_decls` | On-demand rules would be ordered by first appearance, which differs between runs and edits. **Needs a canonical order** for the utility layer (§5 E2). |
| F5 | Declarations are stored as text. `compute()` parses each winning value per element, after `var()` substitution | `computed.rs`, `sheet.rs` `Declaration` | Utilities cost the same per node as hand-written rules. Value parsing, not matching, dominates (§6). |
| F6 | Invalidation: class, id and attribute maps give a `Reach`. State dependencies are a `Vec<(StateMask, Reach)>` keyed by **bit only** | `invalidation.rs` | A plain utility's class has `SELF` reach. One `group-hover:` anywhere makes every hover change reach the hovered node's descendants, whether or not it has `.group` (§5 E6). |
| F7 | Colours go through `cssparser-color` (rgb, hsl, hwb, lab, lch, oklab, oklch, `color()` in sRGB). There is **no `color-mix()`** | `values.rs` `color()` | Tailwind's `/50` opacity modifier compiles to `color-mix()`. **Needs adding** (§5 E4). |
| F8 | `transition` exists only as the shorthand. The longhands were deferred | `OW-CSS.md` "Not in this lane" | `transition-colors duration-150 ease-out` can't compose. **Needs longhands** (§5 E5). |
| F9 | Media features: `width`, `height`, `prefers-color-scheme`, `prefers-reduced-motion`, `prefers-contrast`. There is **no platform feature** | `media.rs` `MediaEnv` | `mac:`/`win:`/`linux:` need one (§5 E7). |
| F10 | `Atom` is `Arc<str>`, not interned | `atom.rs` | Dynamic class names don't leak an interner. Generated rules can be dropped (§4.1 guardrails). |
| F11 | Startup parse plus registration of the egui theme and component sheets takes 227 µs (16 KB). One HMR sheet swap takes 17 µs | `OW-CSS.md` §"CSS parse cost" | Parsing costs about 1 µs per rule. Generating rules as CSS text and parsing them is affordable (§6). |

### 1.3 Metadata already available

| Source | What it holds | Generated into |
|---|---|---|
| `metadata/style/{gtk,qt,appkit,winui}.yaml`, the egui kind table plus `ENGINE_PROPS` (through `spec.rs`) | per toolkit: property name, `via`, value type (egui: 37 length, 24 enum, 8 color, 6 raw, 3 number, 2 insets, 1 each of image, shadow, content, font-weight, transition, bool), **enum keywords**, the **kinds** a setter applies to, `inline_only` | `crates/svelters-style/src/tables.generated.rs`, `packages/style/src/props.generated.js` (`gen_props.py`) |
| `metadata/semantics/egui.json` `style.pseudo_classes` | 27 pseudo-classes with their state bit and negation | from `spec.rs` (`SVELTERS_UPDATE=1 … --test spec`) |
| `egui.json` `kinds[].states` | per kind: which states it can have (name, pseudo-classes, source, "set when") | DOC1's reference pages (`tools/docs/egui/gen_reference.py`) |
| `KindEntry.parts: &[PartEntry { name, docs, node }]` | per kind: its `#[part]`s and whether each is a node or paint part | **only in the Rust registry; not in `egui.json`** |
| `svelters-egui/src/theme/mod.rs` (`color_set!`, `metric_set!`), `theme/data/*.json` | the L2 roles with **the VS Code key each comes from** (`list_hover = "list-hover" <- "list.hoverBackground"`), the L3 component colours and metrics, four colour sets, three densities, shape tokens (radius, spacing, font sizes, motion, metrics) | `:root` token sheet (DS1). Not in any manifest |
| `svelters-style` `media.rs` | the media features | – |

### 1.4 Compiler facts

- **`class:` directive names end at `/`.** `read_tag_name` (a 1:1 port of Svelte's) stops at whitespace, `/`, `>`,
  quotes and `=`. The directive name is everything after the first `:`, with `|` splitting off modifiers
  (`svelte/parse/state/element.rs` lines 965–984 and 699–722). So `class:hover:bg-x={c}` works, and
  `class:w-1/2={c}` and `class:bg-accent/50={c}` don't.
- `lower/engine_css.rs` (CS5g) already runs the engine's parser and table check on egui `<style>`, mapping
  diagnostics to `.svelte` spans. The utility checker reuses that pattern.
- New codes need a row in `codes.rs` (CX1). Fix-its come from `fixits.rs` (`FixHint::Suggest` "did you mean",
  `Remove`).

### 1.5 Where our tokens and Tailwind's scales disagree

| Scale | Tailwind v4 | Ours (DS1 `shape.json`) | Effect |
|---|---|---|---|
| Spacing | `--spacing: 0.25rem` (4px) × any integer: `p-7` = 28px, `p-8` = 32px | `space-1..8` = 4, 8, 12, 16, 20, 24, **32, 40** | The `space-N` tokens match Tailwind's `N` only for 1–6. Utilities use the multiplier, not the `space-*` tokens. |
| Radius | xs 2, sm 4, md 6, lg 8, xl 12, 2xl 16 ... (px at 16px rem) **[unverified]** | `radius-xs` 2, `radius` 4, `radius-md` 6, `radius-lg` 8, `radius-xl` 12 (VS Code's `cornerRadius.*`) | They line up: `rounded-sm` is our `radius`. |
| Font size | xs 12, sm 14, base 16, lg 18, xl 20, 2xl 24 **[unverified]** | `font-size-xs` 11, `-sm` 12, `font-size` 13, `-h3` 13, `-h2` 18, `-h1` 26, `-label1..3` 12/11/10 | Names are kept and values are ours. `text-lg`/`text-xl` don't exist unless the theme adds the tokens. |
| Shadow | 2xs, xs, sm, md, lg, xl, 2xl **[unverified]** | `elev-0..4`. VS Code modernUI uses `shadow-{sm,md,lg,xl}` | `shadow-sm` … `shadow-xl` map onto `elev-*` (§4.5). |
| Colours | an oklch palette (`red-500`, …) | VS Code roles | No palette (§4.5). |

---

## 2. Findings (external)

### 2.1 Tailwind CSS v4

- **Engine.** "Rust where it counts": the scanner and the candidate extractor are Rust (`crates/oxide`), and the
  core stays TypeScript. Lightning CSS is the only dependency.
  [blog/v4-alpha](https://tailwindcss.com/blog/tailwindcss-v4-alpha),
  [crates/oxide/src](https://github.com/tailwindlabs/tailwindcss/tree/main/crates/oxide/src)
  - The extractor is a set of byte-level state machines (utility, variant, arbitrary value, property,
    variable, modifier, string) driven by lookup tables, with no regex. It **over-collects on purpose**:
    "only when we pass the candidates to the core engine … will [we] know for sure if they are valid".
    [PR #16306](https://github.com/tailwindlabs/tailwindcss/pull/16306)
  - There are per-language pre-processors. The Svelte one only rewrites ` class:` to ` class `, so
    `class:px-2.5={c}` yields the token `px-2.5`.
    [svelte.rs](https://github.com/tailwindlabs/tailwindcss/blob/main/crates/oxide/src/extractor/pre_processors/svelte.rs)
- **Speed (Catalyst, v3.4 → v4.0):**

  | Build | v3.4 | v4.0 |
  |---|---|---|
  | Full | 378 ms | 100 ms |
  | Incremental, new CSS | 44 ms | 5 ms |
  | Incremental, no new CSS | 35 ms | 192 µs |

  [blog/v4](https://tailwindcss.com/blog/tailwindcss-v4)
- **CSS-first config (`@theme`).** Theme variables in namespaces create utilities:
  - `--color-*` → `bg-*`, `text-*`, …;
  - `--text-*`, `--font-*`, `--font-weight-*`, `--tracking-*`, `--leading-*`;
  - `--spacing`, `--radius-*`, `--shadow-*`, `--ease-*`, `--animate-*`;
  - `--breakpoint-*` → `sm:`; `--container-*` → `@sm:`.

  Modifiers and resets:
  - `@theme inline` emits the value, not `var()`;
  - `--color-*: initial` clears a namespace, and `--*: initial` clears all of them.

  [docs/theme](https://tailwindcss.com/docs/theme). This is the "utilities are generated from token metadata"
  model, done in CSS; §3 does the same from our metadata.
- **Spacing** is a multiplier: `mt-8` = `calc(var(--spacing) * 8)`, and any integer works (`w-17`).
  [blog/v4](https://tailwindcss.com/blog/tailwindcss-v4)
- **Detection.**
  - Files are read as plain text, and class names must be complete strings (`text-{{ x }}-600` fails).
  - `@source`, `@source not` and `@source inline("{hover:,}bg-red-{50,{100..900..100}}")` add sources,
    exclude them, or safelist classes.
  - [docs/detecting-classes](https://tailwindcss.com/docs/detecting-classes-in-source-files)
- **Variants.**
  - `hover` is wrapped in `@media (hover: hover)` [upgrade guide](https://tailwindcss.com/docs/upgrade-guide).
  - Built in: `focus-visible`, `focus-within`, `active`, `disabled`, `enabled`, `checked`, `invalid`,
    `user-invalid`, `open`, `inert`; `first`/`last`/`odd`/`even`/`nth-*`; `group-*` (named
    `group/item`); `in-*`; `peer-*`; `has-*`; `not-*`; `data-*`; `aria-*`; `motion-safe`/`-reduce`;
    `contrast-more`/`-less`; `forced-colors`; `print`; `supports-*`; `starting:`; `*:` and `**:`; arbitrary
    `[&.is-dragging]:`.
  - Container queries are built in (`@container`, `@sm:`, `@max-md:`).
  - Variants **stack left to right** in v4.
  - Sources: [docs/states](https://tailwindcss.com/docs/hover-focus-and-other-states),
    [blog/v4](https://tailwindcss.com/blog/tailwindcss-v4).
  - `dark:` is `prefers-color-scheme`, or a custom variant. [docs/dark-mode](https://tailwindcss.com/docs/dark-mode)
- **Arbitrary values.**
  - `top-[117px]` (with `_` for a space), `fill-(--my-color)` (a variable shorthand), type hints
    `text-(length:--x)`, and arbitrary properties `[mask-type:luminance]`.
  - Important is a trailing `!`.
  - `/50` opacity becomes `color-mix(in oklab, … 50%, transparent)`.
  - [docs/adding-custom-styles](https://tailwindcss.com/docs/adding-custom-styles),
    [blog/v4](https://tailwindcss.com/blog/tailwindcss-v4)
- **Authoring.**
  - `@utility` (functional: `--value(--tab-size-*)`, `--value(integer)`, `--modifier(…)`);
  - `@variant`;
  - `@custom-variant name (&:where(…))`.
  - [docs/adding-custom-styles](https://tailwindcss.com/docs/adding-custom-styles)
- **Svelte `<style>`.** It can't see the theme: it needs `@reference "../app.css"` before `@apply`.
  [upgrade guide](https://tailwindcss.com/docs/upgrade-guide)
- **Layers and order.**
  - `@layer theme, base, components, utilities`. [blog/v4](https://tailwindcss.com/blog/tailwindcss-v4)
  - Generated rules are sorted by:
    1. a variant bitmask;
    2. the first differing index in `GLOBAL_PROPERTY_ORDER`;
    3. property count, more first;
    4. the name.
  - [compile.ts](https://github.com/tailwindlabs/tailwindcss/blob/main/packages/tailwindcss/src/compile.ts)
- **Size.** "Usually under 10kB" compressed. [docs/file-size](https://tailwindcss.com/docs/controlling-file-size)
- **Editor.** IntelliSense loads the CSS entry's design system (`__unstable__loadDesignSystem`) and asks it for
  `getClassList()`, `getVariants()`, `getClassOrder()`, `candidatesToCss()` and `resolveThemeValue()`.
  Completions come from the compiled theme, not a static list.
  [design-system.ts](https://github.com/tailwindlabs/tailwindcss/blob/main/packages/tailwindcss/src/design-system.ts),
  [intellisense](https://github.com/tailwindlabs/tailwindcss-intellisense)

### 2.2 UnoCSS

- An on-demand atomic engine where every feature is a preset. Tailwind compatibility is one preset
  (`preset-wind3`/`wind4`), and Tailwind plugins are not supported. [guide/why](https://unocss.dev/guide/why)
- **Rules** are static (`['m-1', {…}]`) or `[regex, fn]`. Later rules win.
  [config/rules](https://unocss.dev/config/rules)
- **Speed.** The original benchmark: 1656 utilities in 4.97 ms, against 1249.79 ms for Tailwind v3 JIT. It got
  there with no AST (string concatenation) and a **cache per class name**.
  [reimagine-atomic-css](https://antfu.me/posts/reimagine-atomic-css)
- **Other features:**
  - attributify (`<div m-2 text="sm white">`) [presets/attributify](https://unocss.dev/presets/attributify);
  - variant groups `hover:(a b)`, a source transform
    [transformers/variant-group](https://unocss.dev/transformers/variant-group);
  - shortcuts (named class lists);
  - `safelist`/`blocklist` and magic comments. Dynamic concatenation "won't work".
    [guide/extracting](https://unocss.dev/guide/extracting)
- **Runtime mode** (`@unocss/runtime`): it is the closest analogue to §4.1's design.
  - A `MutationObserver` on the root; added nodes' `outerHTML` and changed attributes are re-extracted.
  - Updates are debounced with `setTimeout(0)` and rewrite each layer's `<style>`.
  - `un-cloak` hides elements until they are styled, because FOUC is possible.
  - Sources: [runtime/src/index.ts](https://github.com/unocss/unocss/blob/main/packages-integrations/runtime/src/index.ts),
    [integrations/runtime](https://unocss.dev/integrations/runtime). Bundle size **[unconfirmed]**.
- **svelte-scoped** puts each component's utilities in its own `<style>` wrapped in `:global()`, and in
  production collapses them into one hashed class per element. It supports `class:`, clsx arrays and `@apply`.
  [integrations/svelte-scoped](https://unocss.dev/integrations/svelte-scoped)

### 2.3 Runtime engines and their cost

- **Twind.**
  - "Ships the compiler, not the CSS", about 13 kB [npm](https://npmjs.com/package/twind).
  - The last real release is 1.0.7 / core 1.1.3 (2023-01-24). Since then only sponsor-image commits, and it
    targets Tailwind v3 [repo](https://github.com/tw-in-js/twind). Treat it as unmaintained.
- **Tailwind Play CDN** is "for development purposes only".
  [docs/play-cdn](https://tailwindcss.com/docs/installation/play-cdn)
- **Emotion** (Spot's measurement): a list render took 54.3 ms against 27.7 ms with Sass modules.
  - The causes are per-render serialisation and hashing, and rule insertion during render that forces a style
    recalculation.
  - [dev.to/srmagura](https://dev.to/srmagura/why-were-breaking-up-wiht-css-in-js-4g9b)
- **Build-time atomic (StyleX, Panda).**
  - Facebook's CSS went from 413 KB to 73 KB, because atomic CSS "has a logarithmic growth curve".
    [engineering.fb.com](https://engineering.fb.com/2020/05/08/web/facebook-redesign/),
    [InfoQ](https://www.infoq.com/news/2020/04/facebook-cssinjs-react-conf-2019/)
  - StyleX: "the last style applied wins", and all CSS is loaded up front because lazy-loading CSS
    "triggers a recalculation of styles for the entire page".
    [stylexjs.com](https://stylexjs.com/docs/learn/thinking-in-stylex/)
  - Panda requires "values visible in source". [panda-css.com](https://panda-css.com/docs/overview/why-panda)

**What applies to us.** The runtime costs on the web come from three things:

1. shipping and running a JS compiler;
2. inserting rules into the CSSOM mid-render, which forces a global style recalculation;
3. per-render hashing.

None of the three exists in svelters:

1. **The engine is already in the binary.** Utilities add the resolver, not a parser.
2. **Adding a rule to our stylist doesn't restyle anything by itself.** Only nodes carrying the new class are
   dirty, and they are dirty anyway because their class list just changed.
3. **The per-class cache is the stylist's own class bucket.**

So the cost argument that pushed the web to build-time extraction doesn't carry over (§6).

### 2.4 Svelte and Tailwind

- Since Svelte 5.16, `class` takes objects and arrays (clsx), and the docs suggest them instead of `class:`
  ("useful for Tailwind"). [svelte.dev/docs/svelte/class](https://svelte.dev/docs/svelte/class)
- **`class:` names stop at `/`**, read from Svelte's parser source
  ([element.js](https://github.com/sveltejs/svelte/blob/main/packages/svelte/src/compiler/phases/1-parse/state/element.js)).
  Our port matches (§1.4).
- **tailwind-merge**: a later class wins within a conflict group, and each variant is kept apart. People install
  it because Tailwind itself resolves conflicts by CSS order, not class order.
  [tailwind-merge](https://github.com/dcastil/tailwind-merge)
- **prettier-plugin-tailwindcss** sorts classes in CSS output order: layout first, variants after plain ones,
  responsive last. The order is fixed.
  [blog/class-sorting](https://tailwindcss.com/blog/automatic-class-sorting-with-prettier)

### 2.5 Native and desktop prior art

- **GPUI (Zed)** is the closest relative. It has a `Styled` trait of Tailwind-named builder methods
  (`.flex().flex_col().p_2().bg(…).text_sm()`), whose doc comments link to tailwindcss.com.
  [styled.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/styled.rs)
  - The methods are **generated by proc-macros from a table of suffixes**: numbers are `rems(n/4)`, `px`,
    `full`, fractions `1_3`, `auto`. [gpui_macros/styles.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui_macros/src/styles.rs)
  - States are typed closures refined in a fixed order: base, focus, hover/group-hover, drag-over, active. There
    is no cascade. [div.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/elements/div.rs)
  - What applies: generating the vocabulary from a table is proven at scale, and a generated typed Rust façade
    for OW-RUST is possible later (§4.6).
- **Qt style sheets** have no utilities. A property change after the sheet is set needs a re-set or
  unpolish/polish [qt docs](https://doc.qt.io/qt-6/stylesheet-syntax.html); CSS.md measured about 1 s per
  1000 widgets for an app-sheet re-polish. Not a model.
- **Slint** has no CSS: compile-time styles plus a `Palette` global. [slint docs](https://docs.slint.dev/latest/docs/slint/reference/std-widgets/globals/palette/)
- **Avalonia** has CSS-like classes and pseudo-classes. Its utility libraries are prototypes:
  - Tailwind.Avalonia pre-generates every permutation;
  - Enx.Atomic.Avalonia generates at build time, after UnoCSS.

  [avalonia docs](https://docs.avaloniaui.net/docs/basics/user-interface/styling/style-classes),
  [Enx.Atomic.Avalonia](https://www.nuget.org/packages/Enx.Atomic.Avalonia)
- **NativeWind (React Native).**
  - v4 is a rewrite for "highly dynamic styles"; v5 sits on `react-native-css` and Tailwind 4.1, inlining values
    at build time and resolving variables at run time.
  - It disables opacity plugins for performance.
  - [v4](https://nativewind.dev/v4/announcement), [v5](https://www.nativewind.dev/v5/guides/migrate-from-v4),
    [differences](https://www.nativewind.dev/docs/core-concepts/differences)
- **twrnc** is a runtime tagged template returning style objects, with a `memoBuster` key for scheme changes.
  [repo](https://github.com/jaredh159/tailwind-react-native-classnames)
- **Tamagui** is a compiler that flattens and extracts atomic styles. [repo](https://github.com/tamagui/tamagui)
- **Dioxus** ships Tailwind in `dx` [0.7](https://dioxuslabs.com/blog/release-070), and Blitz (Stylo, Taffy,
  Parley) renders real CSS [blitz](https://github.com/DioxusLabs/blitz). How well Blitz renders Tailwind
  output is **[unconfirmed]**.
- **Tauri** apps use web Tailwind unchanged. Nothing there applies to a native engine.
- **egui.** `egui_styled` 0.8 has Tailwind-like builders with per-state styles and tokens
  (`docs/notes/egui-ecosystem.md`). It is reference only.

### 2.6 Atomic CSS in browser style engines

- **Rule buckets.** Blink files rules under the rightmost compound's class, id or tag
  [style-calculation.md](https://chromium.googlesource.com/chromium/src/+/HEAD/third_party/blink/renderer/core/css/style-calculation.md),
  and Stylo's `SelectorMap` does the same
  [selector_map](https://doc.servo.org/style/selector_map/struct.SelectorMap.html). Ours is the same (F1).
- **Invalidation.** Blink keeps class, id and attribute invalidation sets, plus a Bloom filter of names with
  self-only invalidation
  [rule_invalidation_data.h](https://github.com/chromium/chromium/blob/main/third_party/blink/renderer/core/css/invalidation/rule_invalidation_data.h).
  Stylo has a rule tree (restyles can "skip selector matching and sorting completely"), a **style-sharing
  cache** and parallel traversal [hacks.mozilla.org](https://hacks.mozilla.org/2017/08/inside-a-super-fast-css-engine-quantum-css-aka-stylo/).
- **Many classes per element** (Griffel, Edge, 10,000 matches):

  | Classes per element | Time |
  |---|---|
  | 1 | 53 ms |
  | 10 | 57 ms |
  | 100 | 89 ms |
  | 1000 | 372 ms |

  It is about linear in class count, and cheap at the 10–20 classes a real element carries.
  [griffel](http://griffel.js.org/react/guides/atomic-css/)
- **Selector type.** Class selectors beat attribute selectors on every engine.
  [nolanlawson.com](https://nolanlawson.com/2022/06/22/style-scoping-versus-shadow-dom-which-is-fastest/)

---

## 3. The central option: generate the utility set from metadata

### 3.1 Three ways to define the set

| | (M) Mechanical, metadata only | (H) Hand-curated Tailwind set | **(G) Metadata + a thin vocabulary** |
|---|---|---|---|
| What it is | one utility per table property and value, named after the property: `background-color-accent`, `padding-top-4`, `cursor-pointer`, `text-transform-uppercase` | UnoCSS-style `[regex, fn]` rules in Rust, written to match Tailwind | a ~90-row YAML gives Tailwind's names and value sources; everything else (keywords, kinds, states, parts, tokens) comes from metadata |
| Familiar to web developers and to the agents writing cadatomic's UI | no; Tailwind muscle memory and every LLM's training data use `px-2 items-center` | yes | yes, for every utility egui can render |
| Coverage | complete by construction | whatever someone wrote; drifts | complete: properties with no vocabulary row are reachable as `[prop:value]` and listed by `--check` |
| Drift from what the engine can render | none | high: utilities for properties egui lacks, keywords the table doesn't accept, missed new properties | none: the generator intersects every row with the table (keywords ∩ enum values, property ∈ table, kind ∈ tags) and fails `--check` on a mismatch |
| Per-kind knowledge (`gap` only on containers, `checked:` only on toggles, `part-thumb:` only on sliders) | yes | no (or hand-kept) | yes |
| Docs, completions, diagnostics | generated | hand-written | generated |
| Tailwind's ambiguous names (`text-` = colour, size or align; `flex` = display *and* the `flex-1` shorthand; `items-`, `self-`, `justify-`) | can't express | yes | yes, in the vocabulary |
| Maintenance | none | a second copy of everything | a vocabulary row per new *namespace* (rare); values, kinds, states, parts, tokens are free |

**Recommendation: (G).** (M) alone would give names nobody types and that the agents writing cadatomic's panels
wouldn't produce. (H) alone recreates the drift CSS.md D1 removed and duplicates the tables. (G) uses metadata
for everything that is data, and a human for the one thing that isn't: Tailwind's naming. This is what Tailwind
v4 itself does: `@theme` namespaces are data, and the utility names are its built-in vocabulary. It is also what
GPUI does: a table of suffixes expanded by a macro.

### 3.2 The pipeline

```text
 metadata/style/utilities.yaml (hand-written vocabulary)        ┐
 metadata/style/tailwind-v4.yaml (Tailwind namespaces we lack)  │
 metadata/semantics/egui.json  (style.properties, pseudo_classes,├─ tools/metadata/style/gen_utilities.py ─┬─ crates/svelters-style/src/utilities.generated.rs
     kinds[].states, kinds[].parts (new), theme (new), media)    │      (--check in the js CI job)         ├─ packages/style/src/utilities.generated.js  (+ .json manifest)
 crates/svelters-style tables (via egui.json)                   ┘                                         ├─ docs/egui/reference/utilities/*.md, and a "Utilities" region per widget page
                                                                                                           └─ packages/style/tailwind/svelters.css (IntelliSense shim, §3.5)
 run time: the active theme's TokenMap (DS1 + cadatomic + user) ──► the resolver's colour, size and font names
```

Two kinds of names are resolved at different times:

- **Static vocabulary** (namespaces, keywords, variants, parts, states) is compiled into `utilities.generated.rs`.
- **Token names** (`bg-<colour>`, `text-<size>`, `h-<metric>`) are resolved **at run time** against the theme the
  host has. Two consequences:
  - cadatomic's own tokens (`--sv-cad-panel`, set through `Theme::with`) become `bg-cad-panel` with no regeneration;
  - a user's imported VS Code theme can't remove a utility, because every set defines every role (a missing role is a
    problem DS1's `Theme` already reports).

  The generator emits the *default* theme's names for docs and completions.

### 3.3 The vocabulary file (sketch)

```yaml
# metadata/style/utilities.yaml: Tailwind v4 names → svelters properties. Checked and expanded by
# tools/metadata/style/gen_utilities.py against each toolkit's style table, egui.json and the theme.
spacing_unit: { token: spacing, default: 4px }        # p-4 = calc(var(--sv-spacing) * 4)
namespaces:                                            # order = canonical cascade order (shorthands first)
  - { name: p,  props: [padding],                       values: [spacing, arbitrary] }
  - { name: px, props: [padding-left, padding-right],   values: [spacing, arbitrary] }
  - { name: pt, props: [padding-top],                   values: [spacing, arbitrary] }
  # … py pr pb pl, m mx my mt mr mb ml (negative: true), gap gap-x gap-y
  - { name: w,  props: [width],  values: [spacing, fraction, sizes: [full, auto, px], metric, arbitrary] }
  - { name: h,  props: [height], values: [spacing, fraction, sizes: [full, auto, px], metric, arbitrary] }
  - { name: size, props: [width, height], values: [spacing, fraction, metric, arbitrary] }
  - { name: flex, props: [flex-direction], keywords: { row: row, col: column, row-reverse: row-reverse, col-reverse: column-reverse } }
  - { name: flex, props: [flex-wrap],      keywords: { wrap: wrap, nowrap: nowrap, wrap-reverse: wrap-reverse } }
  - { name: flex, props: [flex-grow, flex-shrink, flex-basis], keywords: { "1": [1, 1, "0%"], auto: [1, 1, auto], none: [0, 0, auto] } }
  - { name: items,   props: [align-items],     keywords: from-table }   # start end center stretch baseline
  - { name: self,    props: [align-self],      keywords: from-table }
  - { name: justify, props: [justify-content], keywords: { start: start, end: end, center: center, between: space-between, around: space-around, evenly: space-evenly } }
  - { name: bg,     props: [background-color], values: [color, arbitrary] }
  - { name: text,   props: [color],     values: [color] }                    # text-fg
  - { name: text,   props: [font-size], values: [font-size] }                # text-sm  (font-size-* tokens)
  - { name: text,   props: [text-align], keywords: { left: left, center: center, right: right, justify: justify, start: start, end: end } }
  - { name: font,   props: [font-weight], keywords: { thin: 100, light: 300, normal: 400, medium: 500, semibold: 600, bold: 700 } }
  - { name: font,   props: [font-family], values: [font] }                   # font-sans, font-mono
  - { name: rounded, props: [border-radius], values: [radius], bare: radius } # rounded = --sv-radius
  - { name: border, props: [border-width], values: [int-px], bare: 1px }
  - { name: border, props: [border-color], values: [color] }
  - { name: shadow, props: [box-shadow], values: [shadow] }
  - { name: cursor, props: [cursor], keywords: from-table }                 # all 36 egui keywords
  - { name: opacity, props: [opacity], values: [percent-int] }              # opacity-40 = 0.4
  - { name: hidden, props: [display], fixed: none }                         # bare utilities
  - { name: truncate, props: [overflow-x, text-overflow, white-space], fixed: [hidden, ellipsis, nowrap] }
  # …
value_sources:
  color:     { tokens: color }                     # any --sv-* token whose value is a colour in the default sets
  font-size: { tokens: "font-size-*", bare: font-size }   # text-base = --sv-font-size
  radius:    { tokens: "radius-*", aliases: { sm: radius, none: "0", full: 9999px } }
  shadow:    { tokens: "elev-*", aliases: { none: elev-0, xs: elev-1, sm: elev-2, lg: elev-3, xl: elev-4 } }
  metric:    { tokens: ["*-h", "*-w", icon-size] }  # h-row, h-control, w-sash, size-icon (density-aware)
  font:      { aliases: { sans: font-family, mono: font-mono } }
roles_vscode_aliases: true                          # bg-editor → role surface (from the role table)
```

`keywords: from-table` means the generator copies the egui table's enum values (and fails if a Tailwind keyword
listed elsewhere isn't one of them). `tailwind-v4.yaml` lists the Tailwind namespaces we don't provide, each with
a reason and a replacement, for example:

- `ring` → use `outline-*`;
- `rotate`/`scale`/`skew`/`translate` → no `transform` on egui;
- `z` → layers are structural;
- `block`/`inline` → everything is a flex or grid container.

### 3.4 What metadata is missing, and where it should live

| Missing | Why the generator needs it | Where it should live | Size |
|---|---|---|---|
| The **naming vocabulary** (namespace → properties, value source, Tailwind keyword names, canonical order) | Tailwind's names can't be derived | `metadata/style/utilities.yaml`, hand-written, like the toolkit YAMLs | 0.5 d |
| **Tailwind namespaces we lack**, with reason and replacement | `utility_unsupported` diagnostics and docs | `metadata/style/tailwind-v4.yaml`, hand-written once from Tailwind's docs (names are facts; Tailwind is MIT) | 0.5 d |
| **Parts per kind** (name, node or paint, docs) | `part-<name>:` variants, per-kind validity, docs | `egui.json` `kinds[].parts`, dumped by `spec.rs` from `KindEntry.parts` (already in the registry) | 0.25 d |
| **Built-in provider states** (`sv.overflow-*`) | `state-[…]:` validity and docs | `egui.json` `kinds[].provider_states`, from the registry; cadatomic's come through DOC1's `--fragment` | 0.25 d |
| **A token manifest**: each L2/L3 token's name, **type** (colour, length, shadow, duration, easing, font family), the VS Code key it comes from, its doc | colour names, VS Code aliases, value types, docs | `egui.json` `theme.tokens`, dumped by `spec.rs` from `theme/mod.rs` (`color_set!`, `metric_set!`, `COMPONENT_*`) and `theme/data/*.json`. The role table stays the single source | 0.5 d |
| **The spacing unit** `--sv-spacing: 4px` | the multiplier | `theme/data/shape.json` (one token) | – |
| **Breakpoints and container sizes** (`--sv-breakpoint-*`, `--sv-container-*`) | `sm:` … and `@sm:` variants (§4.4) | `theme/data/shape.json` | – |
| **Value ranges** for numeric properties (opacity 0–1, flex-grow ≥ 0, widths ≥ 0) | guardrails on dynamic and arbitrary values; better `style_bad_value` messages | an optional `range:` on table rows (`metadata/style/*.yaml`, egui `ENGINE_PROPS`), carried into `tables.generated.rs` | 0.25 d |
| **Media features** (names and values) | variant list and docs | a `style.media_features` list in `egui.json` from `svelters-style` | 0.1 d |

Nothing here duplicates an existing source. Each new item is either dumped from the Rust registry that already
owns it (parts, states, tokens), or is the irreducible human part (vocabulary, Tailwind gaps).

### 3.5 Generated outputs

| Output | Content | Consumer |
|---|---|---|
| `crates/svelters-style/src/utilities.generated.rs` | `NAMESPACES` (sorted, with canonical rank), keyword maps, value sources, variant tables (state, structural, media, part names with node/paint), `UNSUPPORTED` (name, reason, replacement), per-kind state and part sets | the engine's resolver (run time), the compiler's checker (compile time) |
| `packages/style/src/utilities.generated.js` + `utilities.generated.json` | the same as data | docs build, the IntelliSense shim, SV33's language server later, MCP `describe_utilities` |
| `docs/egui/reference/utilities/*.md` | one page per family (spacing, sizing, flex/grid, colour, typography, borders, effects, interactivity, transitions, variants), each listing the default theme's values per scheme | the docs site (DOC1 conventions: BEGIN/END GENERATED regions) |
| a **"Utilities" region on each widget page** (`gen_reference.py`) | the kind's state variants, its `part-*:` variants (node or paint), the properties that apply to it | DOC1 pages |
| `packages/style/tailwind/svelters.css` (optional) | `@import "tailwindcss/theme" …; @theme { --*: initial; --color-surface: var(--sv-surface); … --spacing: 4px; … }` plus `@custom-variant` for every state, part, platform and `state-` variant | stock Tailwind IntelliSense in VS Code gives completions and hovers for our names now (dev dependency only). It still offers Tailwind utilities we lack; our compiler's diagnostics flag those |
| **d.ts**: nothing new | `ClassValue` stays a string type. A union of utility literals with stacked variants is far beyond TypeScript's union limit, and a space-separated string can't be checked by TS anyway | – |

Later, the same tables can generate a typed Rust façade for OW-RUST (`imm::button(ui).px(2).bg(Role::Surface)`), as
GPUI's macros do. It is not needed for v1, because `imm` builders can take a class string (§4.6).

### 3.6 What the generator checks (`--check` fails on any of these)

- Every vocabulary property exists in the egui table, and its value source fits the property's type.
- Every keyword maps to one of the property's enum values.
- No two vocabulary rows claim the same utility name. This holds across `text-`, where colour, size and align
  are disjoint; the generator proves the default token names and the keywords don't collide.
- Every Tailwind namespace is either in the vocabulary or in `tailwind-v4.yaml`.
- Table properties with no vocabulary row are printed, as a to-do, not a failure. They stay reachable as
  `[prop:value]`.
- Part names that collide with a state variant are reported (`backdrop` is both a state bit and the modal's
  part, which is why parts take the `part-` prefix).

---

## 4. Design

### 4.1 Where utilities are generated

| Option | How | Cost | Dynamic classes | Live compile and `.svb` |
|---|---|---|---|---|
| (a) Compile time | the compiler scans class attributes and literals, as Oxide does, and emits a utility sheet per component or app | scan ≈ free; sheet parse at load ≈ 1 µs/rule | **no**: `p-{n}` and computed strings need a safelist | each live compile emits a sheet; bundles carry it |
| (b) Load time | the module loader scans each module's source and registers a sheet | same as (a), paid on every load | no | the loader path only; Vite needs its own |
| (c) **Run time, in the host engine** | the stylist generates a rule the first time a node carries an unknown class that parses as a utility | ≈ 1–3 µs per *distinct* utility per process (resolve + generate text + parse), then a hash hit | **yes**, anything that reaches `CLASSES` | nothing to do: every path ends at `apply_classes`; bundles carry nothing |
| (d) Hybrid | (c) plus (a) as a pre-warm list | (c) minus first-frame generation | yes | a list in the bundle |

**Recommendation: (c), with the compiler scanning for diagnostics only. (d)'s pre-warm list is built only if
U7's measurement shows first-frame generation above 1 ms.**

- **Mechanism.** `Stylist` gains a utility layer (§5 E2). `apply_classes` passes classes the stylist hasn't seen
  through `Stylist::ensure_utilities(&[Atom])`:
  1. a hash lookup in a known-class set (hits: done);
  2. on a miss, the resolver parses the class (§4.2) against `UTILITIES` and the theme's token names;
  3. it emits CSS text for the rule, for example
     `@layer utilities { .hover\:bg-list-hover:hover { background-color: var(--sv-list-hover) } }`;
  4. `Stylesheet::parse` parses it once, the same parser and checks as any sheet, and the rule is appended
     without a rebuild;
  5. a class that isn't a utility goes into a negative set, so a semantic class like `row` costs one lookup.

  Misses are batched per restyle pass, so 300 new classes on the first frame make one append.
- **Invalidation.** The new rule's class has `SELF` reach (or what its variant implies). The node carrying it is
  already marked by its class change, so no other node is restyled.
- **Dynamic classes.** `class="p-{n}"`, `class={cond ? 'bg-accent' : 'bg-surface'}` and clsx objects all work
  with no safelist. This is the one place we beat Tailwind, whose docs forbid it.

  The guardrail is for *continuous* values (`w-[{px}px]` bound to a drag), which would create one rule per value:
  - the compiler warns (`utility_dynamic_value`, §4.7) when an interpolation sits inside a utility token, and
    suggests `style:width={px}`;
  - the host counts generated utilities and **refcounts** them through the nodes' class lists. Past a high-water
    mark (default 4096) it drops rules no node references, oldest first, and warns once in dev.
- **The compiler** links the same resolver (as CS5g links the engine). It checks every *complete* class token it
  can see:
  - static `class="…"`;
  - `class:` names;
  - string literals inside `class={…}` (clsx arrays and object keys);
  - and, opt-in, string literals in `<script>` (`utilities.scan: "script"`), as Oxide scans whole files.

  It reports §4.7's diagnostics and generates nothing.
- **Release `.svb`.** Unchanged. The bundle has the same class strings, and the host generates on first use.
  Q4b's machine independence is untouched.
- **Extensions** (AS3, later). Their classes share the one utility layer. That is safe because a utility is a pure
  function of its name. Their `@utility` definitions (later, U6) are named global rules, so the definition record
  carries the registering sheet's owner as an opaque, `#[non_exhaustive]` principal id. This leaves room for
  ARCH-SECURITY without building any policy (user, 2026-10-22).
- **Other backends.** egui only, like LD2. GTK and Qt get a one-time warning naming egui. A later port could have
  the same resolver emit CSS text into their translator, but AppKit, WinUI and Qt are maintenance-only.

### 4.2 Class grammar

```text
class     := ["!"] (variant ":")* ["-"] base ["/" modifier] ["!"]      # leading "!" = legacy important
variant   := ident ["-" ident]* ["/" groupname]                       # hover, group-hover/row, part-thumb, not-hover
           | ident "-[" arbitrary "]"                                  # has-[:checked], prop-[orientation=vertical], state-[sv.overflow-top]
           | "@" ident | "@[" length "]" | "@max-" ident               # container (CS-CQ)
           | "[" selector-containing-& "]"                             # [&>row]:…  (underscore = space)
base      := ns ["-" value] | "[" property ":" value "]"
value     := keyword | number | fraction | token-name | "[" arbitrary "]" | "(" "--" ident ")"
modifier  := number | "[" arbitrary "]" | "(" "--" ident ")"           # bg-accent/50, text-sm/6
```

- The prefix option (Tailwind's `tw:`) is supported as a first variant when the embed sets one (§4.6).
- The selector is the class escaped with `cssparser::serialize_identifier` (`Atom` already serialises this way).
- Underscores in brackets become spaces, and `\_` is a literal underscore, as in Tailwind.
- **Ambiguous `text-x`** dispatches in a fixed order: font-size token, colour token, align keyword, overflow
  keyword. The generator proves the default names disjoint. A theme token that collides at run time (a
  colour named `sm`) is reported once.

### 4.3 Compatibility: what maps, what doesn't, and how it's reported

| Family | Utilities (Tailwind v4 names) | egui support | Notes |
|---|---|---|---|
| Spacing | `p* m* gap*` incl. `-mt-2`, `px`, arbitrary | ✓ | `space-x-*` → `:where(& > :not(:last-child))` margin. It works, but the fix-it prefers `gap-*` |
| Sizing | `w h size min-w max-w min-h max-h`, fractions, `full`, `auto`, `px`, metrics (`h-row`, `h-control`, `w-sash`) | ✓ | `screen`/`dvh` units ✗ (no viewport units) |
| Layout | `flex grid hidden`, `flex-row/col/wrap`, `flex-1/auto/none`, `grow shrink basis-*`, `items self justify justify-items`, `grid-cols-N` (`repeat(N, minmax(0,1fr))`), `col-span-N`, `row-span-N`, `relative absolute static`, `inset-* top-* …`, `overflow-*`, `order-*` (with P3) | ✓ | `block`, `inline*`, `contents`, `fixed`, `sticky`, `float`, `columns`, `content-*` (align-content), `place-*` ✗ |
| Colour | `bg-* text-* border-* border-{t,r,b,l,x,y}-* outline-*`, `/NN` opacity, `placeholder:text-*` | ✓ (E4 for `/NN`) | `accent-*`, `fill-*`, `stroke-*`, `caret-*` (until a caret part reads `color`), `decoration-*` ✗ or later |
| Typography | `text-<size>`, `font-<weight>`, `font-sans/mono`, `italic`, `uppercase`, `lowercase`, `capitalize`, `normal-case`, `underline`, `line-through`, `no-underline`, `leading-*`, `tracking-*`, `text-left/center/right`, `truncate`, `text-ellipsis`, `whitespace-*` | ✓ | `line-clamp`, `font-stretch`, `tabular-nums`, `indent-*`, `align-*` ✗ |
| Borders | `border`, `border-N`, `border-{side}`, `border-{side}-N`, `border-dashed`/`dotted`/`solid`/`none`, `rounded*` incl. corners, `outline outline-N outline-offset-N outline-none` | ✓ (dashed is an OW-CSS follow-up) | `divide-*` ✓ via `:where(& > :not(:last-child))`; `ring-*` ✗ → `outline-*` |
| Effects | `shadow-*` (elevation), `shadow-none`, `opacity-N` | ✓ | `inset-shadow-*`, `ring`, `blur`, `drop-shadow`, `backdrop-*`, `mix-blend-*`, filters ✗ |
| Interactivity | `cursor-*` (36 keywords), `scroll-m-*`, `overscroll-*`, `scrollbar-gutter-stable` (ours) | ✓ | `pointer-events-*`, `select-*`, `touch-*`, `resize`, `snap-*` ✗ (props, not CSS: the replacement names the prop) |
| Transitions | `transition`, `transition-colors`, `transition-opacity`, `transition-shadow`, `transition-none`, `duration-N`, `delay-N`, `ease-linear/in/out/in-out`, `ease-motion` (ours, `--sv-ease-out`) | ✓ (E5) | `animate-*` ✗ (Svelte transitions, TR1); `transition-transform` ✗ |
| Backgrounds | `bg-linear-to-{dir} from-* via-* to-*` | later (needs gradient stop variables) | `bg-radial` later (P4); `bg-[url(…)]` ✗ (use `content: url()` on parts) |
| Transforms, z-index, tables, lists, SVG | – | ✗ | reported |

**Reporting** (CX1 codes, §4.7):

- `utility_unsupported` names the reason and, where one exists, a fix-it: `ring-2` → `outline-2`,
  `space-x-2` → `gap-2` (warning only, since it still works), `select-none` → the `selectable` prop.
- `utility_unknown` covers a known namespace with an unknown value (`bg-acent`), with "did you mean" fix-its from
  the default theme's names (`fixits.rs` `Suggest`, at most three).
- A class that matches no namespace is a plain class and is never reported, because components legitimately use
  semantic classes.

**Theme source.** Read our token files; don't read Tailwind `@theme`.

- DS1's `TokenMap` already merges the VS Code base, affordance overlay, density, cadatomic and user layers, with
  four scheme and contrast sets. A Tailwind `@theme` block has one value per name and does dark mode with variable
  overrides.
- Reading it would create a second, weaker token source competing with DS1's.
- Writing one (§3.5's shim) gives the editor benefit with none of that.

### 4.4 Variants

All of these come from metadata except the media and structural lists. Selector shapes follow Tailwind v4's output
(`&` is the utility's class).

| Variant | Emits | Source |
|---|---|---|
| `hover:` `active:` `focus:` `focus-visible:` `focus-within:` `disabled:` `enabled:` `checked:` `indeterminate:` `selected:` `open:` `expanded:` `invalid:` `valid:` `user-invalid:` `read-only:` `read-write:` `required:` `optional:` `default:` `placeholder-shown:` `dragging:` `drop-target:` `current:` `busy:` `exiting:` `backdrop:` `has-children:` | `&:<pseudo>` | `egui.json` `style.pseudo_classes` (27 entries). **`hover:` is not wrapped in `@media (hover: hover)`**: desktop pointers, and touch sets no hover bit |
| `aria-checked:` `aria-expanded:` `aria-selected:` `aria-disabled:` `aria-busy:` `aria-invalid:` `aria-required:` `aria-readonly:` `aria-current:` | the state variant above | aliases. An `info` fix-it rewrites them to the native name |
| `first:` `last:` `only:` `odd:` `even:` `nth-N:` `nth-last-N:` `empty:` | `&:first-child` … | fixed list. After a `part-` variant they test the part's logical position (P1) |
| `group` / `group/name` marker; `group-<state>:` `group-<state>/name:` | `&:is(:where(.group):<state> *)` | the state list. Needs E6 to stay cheap (§6) |
| `peer` marker; `peer-<state>:` | `&:is(:where(.peer):<state> ~ *)` | the state list |
| `has-<state>:`, `has-[sel]:` | `&:has(*:<state>)`, `&:has(sel)` | engine `:has` (ancestor invalidation) |
| `not-<state>:`, `not-[sel]:` | `&:not(:<state>)` | |
| `in-<state>:` | `:where(*:<state>) &` | |
| **`part-<name>:`**, plus `placeholder:` and `selection:` | `&::part(<name>)`, `&::placeholder`, `&::selection` | `egui.json` `kinds[].parts` (new, §3.4) |
| `state-[ns.name]:` | `&:state(ns.name)` | provider states |
| `prop-<name>:`, `prop-[name=value]:` | `&[name="true"]`, `&[name="value"]` (props as attributes, booleans as `"true"`/`"false"`) | the kind's props. `data-*:` gets a fix-it to `prop-` (nodes have no data attributes) |
| `*:` `**:` | `:is(& > *)`, `:is(& *)` | fixed |
| `[sel]:` | `sel` with `&` | arbitrary |
| `dark:` `light:` | `@media (prefers-color-scheme: …)` | media. Prefer tokens; `dark:` is for the rare case |
| `contrast-more:` `contrast-less:` | `@media (prefers-contrast: …)` | media. `forced-colors:` → fix-it to `contrast-more:` (HC is a token set here) |
| `motion-safe:` `motion-reduce:` | `@media (prefers-reduced-motion: …)` | media |
| **`mac:` `win:` `linux:`** | `@media (-sv-platform: macos | windows | linux)` | new feature (E7), set from `native::Policy`'s platform (so tests can emulate it), not `cfg!` |
| `sm:` `md:` `lg:` `xl:` `2xl:`, `max-*:` | `@media (width >= <breakpoint>)` | `--sv-breakpoint-*` tokens, resolved to px when generated, because media queries can't use `var()`. Width is the toplevel's. Documented as the wrong tool for docked panels |
| `@xs:` … `@xl:`, `@max-*:`, `@[400px]:`, `@container`, `@container/name` | `@container (width >= …)`, `container-type: inline-size` | **CS-CQ** (planned, 1–2 d). `--sv-container-*` tokens sized for IDE panels (proposal: 240, 320, 480, 640, 800 px) |
| `print:` `supports-*:` `starting:` `pointer-*:` `inert:` | – | `utility_unsupported` |

**Stacking is left to right, as in Tailwind v4.** Each pseudo variant nests inside the previous one, and media and
container variants hoist outside the rule. So:

- `hover:part-thumb:bg-accent` → `.c:hover::part(thumb)` (the host hovered);
- `part-thumb:hover:bg-accent` → `.c::part(thumb):hover` (the thumb's own state, for node parts; for paint parts
  the engine reads the host's state, DOC2);
- `group-hover:part-close:opacity-100` → `.group:hover .c::part(close)`;
- `dark:hover:bg-x` → `@media (prefers-color-scheme: dark) { .c:hover {…} }`.

**Part syntax: `part-<name>:`, not a bare part name.** Bare names collide with state variants (`backdrop` is both
bit 20 and the modal's part) and with future Tailwind variants. `part-` reads like Tailwind's own `group-`/`peer-`.
Tailwind's arbitrary form `[&::part(thumb)]:` is accepted too, with a fix-it to `part-thumb:`.

**Per-kind validation.** When the compiler knows the tag, it flags:

- a state variant the kind never sets (`checked:` on `box`);
- a part the kind lacks (`part-thumb:` on `button`), or a part state after a paint part;
- a property that doesn't apply (`gap-2` on a leaf).

These are warnings: rules that "never match" are legal CSS, and `style_as` kinds inherit their target's lists.

### 4.5 Theming

- **Colour names.** `bg-<name>`, `text-<name>`, `border-<name>` and `outline-<name>` accept, in this order:
  1. **L2 roles and L3 component colours**: `surface`, `bg`, `fg`, `muted`, `accent`, `list-hover`,
     `button-bg-hover`, …;
  2. **VS Code key aliases derived from the role table**: a role whose source key is
     `<area>.<state?><Background|Foreground|Border>` gets the alias `<area>[-<state>]`, so `bg-editor` = role
     `surface` (`editor.background`), `text-foreground` = `fg`, `bg-sidebar` = `bg`, `border-input` =
     `control-border`. The generator emits the alias list and rejects collisions;
  3. **any other `--sv-*` token whose value is a colour** in the active theme (cadatomic's `--sv-cad-*`).

  Each emits `var(--sv-<role>)`, so the scheme, HC sets, the affordance overlay (which overrides L2 values such
  as `control-border`) and ancestor overrides all apply.

  L1 VS Code keys with no role are reached as `bg-(--vscode-list-dropBackground)`. That isn't linted, since it
  is a theme variable, but the docs say it bypasses the affordance layer.
- **No Tailwind palette** (`red-500` etc.). It would bypass themes and HC. A data colour (net, layer, part colour
  in an MCAD/ECAD view) belongs in `style:` or a token. If cadatomic wants a categorical palette, it adds tokens
  (`--sv-cat-1..12`) and they become `bg-cat-3` automatically.
- **Scales:**

  | Scale | Values |
  |---|---|
  | Spacing | `calc(var(--sv-spacing) * N)` with `--sv-spacing: 4px`, any N (and `0.5` steps). Fixed in every density: density changes semantic metrics (`h-row`, `h-control`, `px-button`), not the unit (DESIGN §2.6: density is a token set, not CSS math) |
  | Radius | `rounded-none` 0, `rounded-xs` = `--sv-radius-xs`, `rounded-sm` and bare `rounded` = `--sv-radius`, `rounded-md`/`lg`/`xl` = `--sv-radius-*`, `rounded-full` 9999px |
  | Font size | `text-xs` = `--sv-font-size-xs` (11px), `text-sm` (12px), `text-base` = `--sv-font-size` (13px), `text-h1`/`h2`/`h3`, `text-label1`/`2`/`3`; others only if the theme defines them. `text-sm/6` sets `line-height` from the spacing unit |
  | Shadow | `shadow-none` = `elev-0`, `shadow-xs` = `elev-1` (the R1 lip), `shadow-sm` = `elev-2` (VS Code `shadow-sm`), `shadow-lg` = `elev-3` (menus, VS Code `shadow-lg`), `shadow-xl` = `elev-4` (dialogs, VS Code `shadow-xl`), plus `shadow-elev-N` |
  | Motion | `duration-fast` = `--sv-motion-fast`, `duration-motion` = `--sv-motion`, `duration-N` = N ms, `ease-motion` = `--sv-ease-out` |
  | Metrics (density-aware) | `h-row`, `h-control`, `h-tab`, `h-header`, `h-statusbar`, `w-sash`, `w-scrollbar`, `size-icon`, `pl-indent` |

- **Guardrails on arbitrary values:**
  - Lengths: `p-[13px]`, `w-[calc(100%-8px)]`. Checked against the property's type by the same `parse_value`.
    A bad value is an error at compile time and a one-time dev warning at run time.
  - **Colour literals** (`bg-[#ff0000]`, `text-[rgb(…)]`): `utility_arbitrary_color`, a warning by default. Under
    a strict profile (`utilities: { strict_tokens: true }`, which cadatomic's lint profile sets, matching its
    "hard-coded colour is a lint error" rule) it is an error.
  - `(--var)` forms are allowed: they are tokens.
  - Arbitrary properties `[prop:value]`: only properties in the toolkit's table; anything else is
    `utility_unsupported`.
  - Numeric ranges from the table (§3.4) bound `opacity-N` (0–100) and reject negative widths.

### 4.6 Integration

- **Cascade order.**

  ```text
  theme origin: @layer sv.tokens < sv.base < sv.affordance < cad (cadatomic theme)
  author origin: @layer components < utilities  <  unlayered: every component <style> (scoped)
  inline (style=, style:) ; then !important in reverse
  ```

  Utilities are author-origin and layered below unlayered component styles, as Tailwind v4 does: plain CSS beats
  utilities, and utilities beat the theme. So:
  - a utility always overrides the theme's `base.css` (a different origin);
  - a component's own scoped rule on the same element wins over a utility the caller passed;
  - `p-4!` forces it.

  The alternative (a utility origin above author rules) would make `bg-surface` beat the component's
  `.row:hover {…}` (state rules couldn't override a base utility), which is the inline-style trap. The theme's
  `@layer` statement gains `components, utilities`, so the order is fixed before any sheet loads (layer ranks are
  global by first appearance, `cascade.rs` `layer_ranks`).
- **Order inside the layer** is canonical, not by first appearance (E2). Ties after specificity are broken by:
  1. media/container variant rank (none < dark/contrast/motion/platform < breakpoints ascending < containers
     ascending);
  2. the vocabulary row (shorthands first: `p` < `px` < `pt`; `border` < `border-x` < `border-t`);
  3. the class string.

  This is Tailwind's sort (§2.1) without its property-order table. Two utilities for the same property under the
  same variants in one static class list (`p-2 p-4`) get `utility_conflict` with a fix-it removing the loser.
  There are no class-order semantics, unlike tailwind-merge: class order in clsx objects isn't meaningful.
- **Opt-in.** Utilities are off unless the embed turns them on: `EmbedConfig::utilities: Utilities { enabled,
  prefix: Option<&str>, strict_tokens }`, plus `Theme`-level defaults, with cadatomic turning it on.
  - Existing apps' semantic classes (`flex`, `hidden`, `border`, `shadow`, `truncate`, `row`?) must not change
    meaning silently.
  - With `prefix: Some("tw")`, only `tw:`-prefixed classes are utilities.
  - The compiler warns `utility_shadowed_class` when a scoped selector names a class that is also a utility.
- **`class:` directive.** It works for every utility without `/` (`class:hover:bg-list-hover={x}` is fine; the name
  splits only at the first `:`). `class:w-1/2` and `class:bg-accent/50` fail in Svelte's parser. We keep the parser
  1:1, and the parse error gains a fix-it: `class={{ 'w-1/2': x }}`.
- **clsx and dynamic strings** work at run time (§4.1). The compiler checks only complete literals.
- **`@apply` in `<style>`.** The compiler expands it *before* scoping (`scoped_css.rs`, before `compile_css`),
  using the same resolver. Tailwind's `@reference` isn't needed, because the vocabulary is built in and the
  theme's names are known.
  - `@apply p-2 bg-surface hover:bg-list-hover;` becomes the declarations plus nested
    `&:hover { … }`, which the Svelte CSS port already flattens.
  - Media variants become nested `@media`.
  - Part variants become `&::part(x)`.
  - Unknown names are an error (`utility_apply_unknown`).
  - The output is ordinary scoped CSS in the component's unlayered rules, so the cascade is unchanged. The live
    loader runs the same compiler, so `@apply` works there and in bundles.
- **`@utility` and `@custom-variant`** (Tailwind v4 syntax), in **theme-origin sheets only** (cadatomic's
  `@layer cad` sheet), in U6:
  - static `@utility toolbar-item { … }` registers a named utility usable with every variant;
  - `@custom-variant sketch (&:where(:state(cad.sketch) *))` registers a variant.

  Functional `--value()` is deferred. This is UnoCSS's "shortcuts" need, done in CSS.
- **HMR: class-only edits without a remount** (U5). Today a markup edit is an H1 component swap: the instance's
  natives are rebuilt and H3 keeps `$state`. That is a remount of the component's nodes. The new path:
  1. In dev with `hmr`, the compiler moves every **static** `class` attribute value into a sibling module
     `svelters-classes:<id>` that exports a reactive array (`$n.hotClasses(id, ["px-2 bg-surface", …])`) and
     self-accepts.
  2. The component imports it and reads `classes[i]` where it would inline the literal.
  3. The loader's `invalidate` already compiles the changed component to separate style-only edits (LD2). It also
     recognises "only class strings changed" (equal output with the class table blanked) and returns just the
     classes module.
  4. Re-running that module sets the array's entries. The template effects re-run, `applyEngine` sends `CLASSES`,
     the host generates any new utilities and restyles those nodes: same handles, no `Mounted` event.

  Release builds inline the literals and pay nothing. Class edits inside expressions stay component swaps, with
  state kept.
- **Theme edits.** Changing tokens (DS1 `set_theme`) restyles as today. A token rename (rare) makes stale colour
  names unknown, and the dev warning names them.
- **Rust chrome (OW-RUST).** `imm` builders compute styles for a synthetic element (tag plus classes), so they gain
  `.class("px-2 rounded-sm hover:bg-toolbar-hover")` for free. A typed GPUI-style façade generated from the same
  tables is possible later (§3.5).

### 4.7 Diagnostics (new CX1 rows)

| Code | Severity | When | Fix-it |
|---|---|---|---|
| `utility_unknown` | warning | a known namespace with an unknown value (`bg-acent`, `p-x`) | did you mean (≤ 3, from the default theme) |
| `utility_unsupported` | warning | a Tailwind v4 utility or variant egui can't render, from `tailwind-v4.yaml` | the replacement where one exists (`ring-2` → `outline-2`, `forced-colors:` → `contrast-more:`) |
| `utility_variant_never_matches` | warning | a state, part or part state the element's kind never has | – |
| `utility_not_applicable` | warning | a property that doesn't apply to the kind (table `tags`) | – |
| `utility_conflict` | warning | two utilities, one property, same variants, one static list | remove the one that loses |
| `utility_arbitrary_color` | warning (error with `strict_tokens`) | a colour literal in `[...]` | – (the nearest role is a guess; none offered) |
| `utility_dynamic_value` | warning | an interpolation inside a utility token (`w-[{px}px]`, `p-{n}`) | suggest `style:<prop>={…}` |
| `utility_shadowed_class` | warning | a scoped selector's class is also a utility name | – |
| `utility_apply_unknown` | error | `@apply` of a name the resolver rejects | did you mean |
| `utility_alias` | info | `aria-checked:`, `[&::part(x)]:` and other accepted spellings | the canonical spelling |

Each gets a `codes.rs` row (CX1 requires it) and `cx1_check` fix-it tests. At run time (dev), a bad-value utility
warns once through the host's diagnostics, and the count of generated rules is in the stylist's stats (MCP
`computed_style` can show which utility rule set a value).

---

## 5. Engine work in `svelters-style` (sized)

| # | Item | Why | Size |
|---|---|---|---|
| **E1** | `utility` module: the class grammar (§4.2), namespace lookup in `UTILITIES`, value resolution against a `TokenNames` view of the theme (supplied by the backend), CSS text emission; shared by host and compiler | the resolver | 2.5–3 d |
| **E2** | Utility layer in `Stylist`: `ensure_utilities(&[Atom]) -> Vec<Diagnostic>` with known and negative sets, **incremental append** (insert into `RuleIndex` and `Invalidation` without `rebuild()`), a **canonical order key** for the layer's entries (`order` becomes `(u32 base, u64 canonical)`), refcount and high-water GC, stats | F3, F4 | 1.5 d |
| **E3** | **Part-rule index**: bucket pseudo-element rules by part name (and `Placeholder`/`Selection`), then by the host compound's class or tag; `has_pseudo_rules` becomes a map lookup | F2. Benefits theme part rules today | 0.75 d |
| **E4** | `color-mix(in srgb | oklab, a p%, b)` in `values.rs` colour parsing (after `var()` substitution) | `/50` opacity (Tailwind uses oklab) | 0.5 d |
| **E5** | `transition-property`, `-duration`, `-timing-function`, `-delay` longhands, cascaded with the shorthand as OW-CSS's shorthand machinery does | composable `transition-colors duration-150` | 0.75 d |
| **E6** | **Qualified state invalidation**: state dependencies keyed by (bits, the compound's classes or tag), as Stylo does, so `group-hover` restyles descendants only of `.group` nodes | F6. Only once `group-*`/`peer-*` ship | 1 d |
| **E7** | Media feature `-sv-platform` in `MediaEnv`, set by the backend from `native::Policy` | `mac:`/`win:`/`linux:` | 0.25 d |
| **E8** | Atomic-rule fast path: rules whose selector is one class plus state pseudo-classes keep a precomputed `(class, required, forbidden)` state mask and skip `matches_selector`; non-`var()` declarations keep a pre-parsed value | measured first (§6) | 1 d |
| **E9** | Style-sharing cache: siblings with the same tag, class list, state, inline block and parent style reuse a `ComputedStyle` (an `Arc`) | measured first: big realised tables on theme switch | 1.5–2 d |
| – | `@container` (CS-CQ) | container variants | planned elsewhere (1–2 d) |
| – | `aspect-ratio`, `order`, `align-content` in the egui table | `aspect-*`, `order-*`, `content-*` | 0.25 d each, with their layout lanes |

E1–E5 and E7 are needed for v1 (about 6.5 d). E6 comes with `group-*`, and E8 and E9 only if U7's numbers ask
for them.

---

## 6. Performance: does atomic CSS need a fast path?

**Matching: no.** With one rule per utility class, the index (F1) gives each node exactly as many candidates as it
has utility classes, typically 5–15, and each candidate's selector is one class (already proven by the bucket)
plus maybe a state test. That is the best case for a `SelectorMap` design. Griffel's browser numbers put 10
classes at about the cost of 1 (§2.6).

**Invalidation: no for plain utilities, yes for `group-*`.** A utility class has `SELF` reach, so toggling `bg-x`
restyles one node. Hover restyles only what the state map names. The unqualified state map (F6) is the one
real hazard. E6 fixes it before `group-*` ships, or `group-*` ships with E6.

**Insertion: yes.** F3's full rebuild per `add_sheet` would cost O(rules) per new utility. E2's incremental append
and per-pass batching are required.

**Compute: unchanged, and it is the real cost.** Every winning declaration is parsed per element (F5), and token
utilities go through `var()` first. An estimate, not measured: 15 declarations at roughly 0.3–1 µs each is
5–15 µs per node. That is the same as hand-written CSS with the same declarations.

- A hover restyles one or two nodes (≪ 0.1 ms).
- A theme switch restyles every realised node: SG5's virtual lists realise only visible rows, about 50–100 rows of
  about 10 nodes each, roughly 5–15 ms. That is the case E8 and E9 address.

**Generation:** about 1 µs per rule to parse (F11) plus resolve and format. A first frame that discovers 500
distinct utilities pays about 1–2 ms once per process, and later frames pay nothing.

**Measurement plan (lane U7, release build, thinkpadx1 or the WSL PC):**

1. A gallery page with a 2k-row virtual list and a property grid, each row about 12 utilities with `hover:`,
   `part-*:` and `group-hover:` variants.
2. Record: first-frame generation time; restyle time per pass (initial, hover move, scheme switch); bytes per
   generated rule; `compute` share.
3. Thresholds:
   - first-frame generation ≤ 1 ms, else build (d)'s pre-warm list;
   - scheme switch ≤ 8.3 ms (the ZED-LESSONS frame budget the paint already uses), else E9;
   - hover pass ≤ 0.2 ms, else E8.

---

## 7. Lanes

Sizes are in lane-days. "Needs" lists hard dependencies. Every lane follows CLAUDE.md's build rules: `check -p`,
filtered tests, CI as the gate.

| Lane | Content | Needs | Size | Machine (proposal) |
|---|---|---|---|---|
| **U0** metadata and generator | `spec.rs` dumps `kinds[].parts`, provider states, `theme.tokens` and media features into `egui.json`; `--sv-spacing`, `--sv-breakpoint-*`, `--sv-container-*` tokens in `shape.json`; optional `range:` on table rows; `utilities.yaml`, `tailwind-v4.yaml`; `gen_utilities.py` (+ `--check` in js CI) writing `utilities.generated.{rs,js,json}`; reference pages and the per-widget "Utilities" region | – | 3 | any (Python + one `spec.rs` hunk) |
| **U1** resolver and utility layer | E1, E2, E4 in `svelters-style`; unit tests over the grammar and every vocabulary row | U0 (can start on a hand stub of the generated table) | 5 | WSL |
| **U2** engine prerequisites | E3 (part index), E5 (transition longhands), E7 (platform media) | – | 1.75 | any, now |
| **U3** egui wiring | `apply_classes` → `ensure_utilities`; `EmbedConfig::utilities` (enabled, prefix, strict_tokens); token names from `Theme`; `@layer components, utilities` in the theme; GTK/Qt warning; `imm` `.class()`; gallery page "utilities" | U1, U2 | 1.5 | WSL |
| **U4** compiler | the checker (static class, `class:` names, clsx literals, opt-in script scan) with §4.7's codes and fix-its; per-kind validation; `@apply` expansion before scoping; the `class:…/` fix-it | U0, U1 | 3.5 | macbook |
| **U5** hot class table | the `svelters-classes:` module in dev+hmr, loader detection in `invalidate`, the runtime array, tests | U3 | 2 | thinkpad |
| **U6** variants v2 | E6, then `group-*`/`peer-*`/`in-*`; `@utility` and `@custom-variant` in theme sheets (principal id kept); container variants when CS-CQ lands | U3 (+ CS-CQ) | 2.5 | any |
| **U7** measurement | §6's bench and thresholds; E8/E9 only if crossed | U3 | 1 (+ 1–3.5) | thinkpad |
| **U8** editor support | `packages/style/tailwind/svelters.css` IntelliSense shim, docs guide "Utility classes", manifest wired for SV33 | U0 | 1 | any |

Critical path: U0 → U1 → U3 → U5/U6. U2 can start now. Total about 21 lane-days, plus up to 3.5 more if U7
crosses a threshold. cadatomic can use utilities after U3, with diagnostics after U4.

### Acceptance tests

- **U0:**
  - `gen_utilities.py --check` is clean;
  - a `test_gen_utilities.py` fixture proves the checks of §3.6 fire: a vocabulary row with a missing property,
    a non-enum keyword, a name collision, an unlisted Tailwind namespace;
  - `spec.rs`'s `kind_table_is_consistent` covers `parts` (every `PartEntry` appears and node or paint is right);
  - the reference pages regenerate with no diff in CI.
- **U1** (`svelters-style/tests/utilities.rs`):
  - every vocabulary row resolves for one value and emits CSS that `Stylesheet::parse` accepts with no
    diagnostics;
  - variant stacking order (`hover:part-thumb:` vs `part-thumb:hover:`, `dark:hover:`);
  - `bg-editor` resolves to `var(--sv-surface)`;
  - `/50` gives the mixed colour;
  - `p-4!` is important;
  - arbitrary values are type-checked;
  - **order independence**: the same class set discovered in two different orders gives identical cascades;
  - adding 1000 utilities one batch at a time never calls `rebuild` (a counter);
  - the negative cache;
  - GC drops only unreferenced rules.
- **U2:**
  - a part rule matches through the bucket, and a part rule on another part name or host class is never tried
    (a counter);
  - transition longhands cascade with the shorthand (OW-CSS's tests extended);
  - `@media (-sv-platform: macos)` follows `MediaEnv`.
- **U3** (kittest, `svelters-egui/tests/utilities.rs`):
  - `<box class="p-4 bg-surface rounded-md">` has the computed values;
  - `hover:bg-list-hover` restyles **in the same frame** (as `style.rs::hover_restyles_in_the_same_frame`);
  - `part-thumb:bg-accent` reaches the slider's thumb;
  - a scheme switch recolours;
  - a scoped `<style>` rule beats a utility on the same element, and `!` beats it back;
  - a utility beats `base.css`;
  - `class="p-{n}"` follows `n`;
  - with utilities off, `class="flex"` is a plain class;
  - with `prefix: "tw"`, only `tw:p-4` applies;
  - a `.svb` bundle styles identically;
  - an `imm` builder with classes paints the same as the Svelte node.
- **U4** (`svelters-compiler/tests/utilities.rs`, `cx1_check`):
  - each §4.7 code fires on a minimal component, and its fix-it, applied, clears it;
  - `checked:` on `box` warns, and on `checkbox` doesn't;
  - `@apply hover:bg-list-hover` scopes like hand-written CSS (byte-compare with an equivalent `<style>`);
  - no diagnostics on the gallery and cadatomic fixtures;
  - the codes page regenerates.
- **U5** (`svelters-egui/tests/live_styles.rs` pattern):
  - after a click (state 1), editing a static class string re-runs only `svelters-classes:` modules;
  - the node has the same handle, `count 1` is kept, the new utility applies, and there is no
    `Mounted`/`Faulted`;
  - an expression edit is still an H1 swap with state kept;
  - release output has no classes module.
- **U6:**
  - `group-hover:` on a row child restyles when the row is hovered;
  - hovering a node that isn't a `.group` restyles no descendants (an invalidation counter, E6);
  - `@utility` and `@custom-variant` from a theme sheet work with variants;
  - a container variant flips with the panel width (after CS-CQ).
- **U7:** the bench prints §6's table, and the milestone record states which thresholds held.
- **U8:** with the shim, the IntelliSense language server (run in CI once, headless, if cheap; else a documented
  manual check) completes `bg-list-hover` and `part-thumb:`.

---

## 8. Decided (design, 2026-10-22)

These are implementation choices inside the recommended design. The coordinator may revise them.

- The vocabulary is Tailwind v4's. Values come from the theme, and unsupported namespaces are data
  (`tailwind-v4.yaml`) feeding diagnostics.
- Utilities are generated by the engine in the host. The compiler only checks. Bundles carry nothing.
- Order inside the utility layer is canonical (variant rank, vocabulary row, name), never insertion order.
- `hover:` is not media-wrapped. Variants stack left to right. Part variants are `part-<name>:`.
- Spacing is a 4px multiplier in every density. Density reaches utilities only through metric tokens.
- No Tailwind palette. Arbitrary colour literals are linted.
- egui only. GTK and Qt warn.
- No d.ts class typing. Completions come from the IntelliSense shim now and SV33 later.

---

## 9. Decided (user, 2026-10-22)

- **UQ1 + UQ2:** (G) metadata plus a Tailwind-name vocabulary (`metadata/style/utilities.yaml`, everything else generated by `gen_utilities.py` with `--check`); rules generated at run time in the host's Rust style engine on first use; the compiler only checks and reports.
- **UQ3 + UQ8:** utilities sit below a component's own `<style>` rules (Tailwind v4 order), `!` forces; opt-in per embed with an optional prefix.
- **UQ4 + UQ5:** **our tokens only** (the user chose this over the recommendation): colours are our theme roles (no Tailwind palette, follow VS Code themes and high contrast); spacing and type use our own DESIGN tokens under our own names (`space-7` = 32px), not Tailwind's 4px multiplier or Tailwind's type-scale names. Lanes U0/U1 derive the scale names from the token files rather than the Tailwind vocabulary rows.
- **UQ6 + UQ7:** arbitrary lengths allowed and type-checked, colour literals linted (an error under the strict profile cadatomic uses); parts through a `part-<name>:` variant stacked left to right.
- Coordinator (2026-10-22): the editor-completion shim (U8) may add a `tailwindcss` devDependency; U5 (HMR) does not wait for ID1/LI2.
- This supersedes §1.5's "Utilities use the multiplier", §4.5's spacing and font-size rows, and §8's "Spacing is a
  4px multiplier": no `--sv-spacing` token is added. U0 (`plans/milestones/U0-utility-metadata.md`) implements it.

## 10. Open questions for the user (answered above)

Each question has a recommendation. Lanes proceed on the recommendation until answered.

| # | Question | Recommendation |
|---|---|---|
| **UQ1** | Adopt a utility-class system, **generated from metadata plus a hand-written Tailwind-name vocabulary** (§3, option G), rather than a purely mechanical set (M: `padding-top-4`) or a hand-curated Tailwind clone (H)? | **(G).** Metadata supplies every value, keyword, kind, state, part and token, so nothing drifts. The vocabulary supplies the names web developers and LLM agents already write. |
| **UQ2** | Generate utility rules **at run time in the host engine, on demand** (with the compiler only checking), instead of compile-time sheets as Tailwind does? | **Run time in the host.** It is the one path that covers Vite, the live loader, `.svb` bundles, extensions, dynamic `p-{n}` strings and Rust `imm` chrome. The engine is native and resident, so the web's runtime costs don't apply (§2.3, §6). |
| **UQ3** | Cascade position: utilities **below** unlayered component `<style>` rules (Tailwind v4: an author layer; `!` to force), or **above** them (utilities as "inline styles with variants")? | **Below (Tailwind v4).** Above would make a base utility beat a component's own `:hover` rule, the inline-style trap. Component authors keep control of their components, and callers use `!` when they mean it. |
| **UQ4** | Colour utility names: **DS1 roles** (`bg-surface`, `text-fg`, `bg-list-hover`) **plus VS Code-key aliases derived from the role table** (`bg-editor`, `text-foreground`), with no Tailwind palette and no shadcn aliases? | **Yes.** Roles keep the affordance overlay and cadatomic overrides in the path, and the VS Code aliases come free from the role table. shadcn names (`bg-muted` is a background there, a text colour here) would mislead, so they get "did you mean" fix-its only. |
| **UQ5** | Scales: Tailwind's **4px spacing multiplier** (`p-7` = 28px) rather than our `space-N` tokens (`space-7` = 32px); **our** font sizes under Tailwind's names (`text-base` = 13px, not 16px)? | **Yes to both.** Spacing numbers then mean what every Tailwind user expects. Font sizes must follow VS Code's metrics, and Tailwind's own `@theme` model says names come from the theme. |
| **UQ6** | Arbitrary values: allow `p-[13px]`/`w-[calc(…)]` freely, and **lint colour literals** (`bg-[#f00]`) as a warning, an error under cadatomic's strict profile? | **Yes.** Lengths are checked by type. Colour literals are the one thing that breaks themes and high contrast. |
| **UQ7** | Part variant syntax `part-<name>:` (plus `placeholder:`/`selection:`), stacking left to right, so `hover:part-thumb:` means the host is hovered and `part-thumb:hover:` means the thumb is? | **Yes.** Bare part names collide with state variants (`backdrop`). `part-` reads like `group-`/`peer-`, and left to right is Tailwind v4's rule. |
| **UQ8** | Opt-in per embed (`EmbedConfig::utilities`, optional prefix), not on for every app? | **Opt-in.** Existing semantic classes (`flex`, `hidden`, `border`) must not change meaning silently. cadatomic turns it on. |

**For the coordinator** (not user questions):

- lane order (U2 can start now; U0 and U1 are the critical path);
- whether U8's IntelliSense shim is worth a dev dependency on `tailwindcss` (recommended: yes, devDependency of
  `packages/style` only);
- whether U5 (hot class table) waits for ID1/LI2 (recommended: no; it needs only LD2's detection pattern).
