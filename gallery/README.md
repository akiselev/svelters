# The skelter gallery's stories

Every skelter element kind, in every variant a polisher needs to see, as plain Svelte 5 components. The
`skelter-gallery` CLI (`crates/skelter-gallery`) renders them headlessly to PNG or JSON; the gallery app
(`crates/skelter-gallery-app`) shows them live, with the source of each variant editable beside it. The spec
is `plans/cloud/gallery-b1/BRIEF.md`; the coverage list (every kind, its story, or why it has none) is in
`plans/milestones/GAL2-stories.md`.

This directory is a svelters source package (`package.json`, `"svelters": { "toolkit": "skelter" }`), so
`SourceConfig::for_path` finds its root from any story file. Its dependencies are `svelte`,
`@svelters/native-runtime` and `@svelters/lists` (for `<ListView>` and `<TreeView>`); run
`pnpm install --frozen-lockfile` at the repository root once.

## Layout

```
gallery/
  package.json
  stories/<family>/<id>/
    story.toml
    <variant>.svelte      # one self-contained component per variant: what the gallery renders and shows
    *.svg                 # assets a variant imports, next to it
```

Families: `controls`, `text`, `pickers`, `views`, `menus`, `tabs`, `dialogs`, `layout`, `data`, `feedback`.
A family directory exists only when it has a story.

## `story.toml`

```toml
id = "button"                 # stable id: lowercase and `-`; never a label
title = "Button"              # display text
family = "controls"
kinds = ["button"]            # the registry kinds this story covers (the coverage check reads it)
size = [360, 72]              # each cell's logical size in px
states = ["normal", "hover", "active", "focus-visible", "disabled"]   # optional; this is the default set
notes = "What to look at."    # optional

[[variant]]
id = "look"
title = "Emphasis"
file = "look.svelte"
args = { look = ["plain", "default", "frameless"] }   # optional matrix: each combination is passed as props
```

`kinds` names the kinds as the registry names them, which is the tag except for two:
`<combo-box>` is the kind `select` and `<action>` is `menu-item`. A story may cover several kinds
(`rich-text` covers the five inline runs) and several stories may cover one (`text-edit`, `password` and
`multiline`). `kinds = []` is a story about a common prop rather than a kind (`tooltip`).

## Variants

A variant file is a plain Svelte 5 component that renders on its own: no app state, no network, no clock
(an animating kind is rendered at a fixed virtual time). It takes its matrix args as props, with the first
value of each as its default:

```svelte
<script>
  let { look = 'plain', disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <button anchor="subject" text="Save changes" frame={look !== 'frameless'} enabled={!disabled} />
</box>
```

- Use the kinds' real props only (`metadata/semantics/skelter.json`); `svelters-compile --backend skelter
  --check <file>` rejects an unknown element or prop. Text is plain English sample text, plus Arabic or
  Hebrew in the `rtl` variants.
- Put a root `box` with some padding around the subject, so a focus ring or a shadow is not clipped by the
  cell.
- Cover what a polisher needs: each visual variant the kind supports, long and empty text, right-to-left
  text where the kind lays out text, overflow, and the kind inside a dense layout.

## The `subject` anchor

States are forced on the node marked `anchor="subject"`; several nodes may carry it. With no subject, states
apply to the variant's root node. Selectors (`skelter-gallery inspect … '#subject'`) are anchors or kind
names, never visible text. Inline runs (`<span>`, `<b>`, `<i>`, `<code>`, `<a>`) take no `anchor`; mark
their `label`.

## States and `disabled`

A cell is (story, variant, args combination, state, theme, density, zoom). The states are `normal` (forces
nothing), `hover`, `active`, `focus`, `focus-visible`, `checked` and `disabled`; a story lists the ones
that mean something for it in `states`.

`disabled` is a prop, not a forced bit: declare `disabled = false` in `$props()` and pass
`enabled={!disabled}` to the subject. The gallery then passes `disabled = true`, so the kind's real disabled
state and its accessibility state are what render. A variant without the prop gets the `disabled` bit forced
instead, and its cell's label says so.

Themes are VS Code's `dark-modern`, `light-modern`, `hc-dark` and `hc-light`; densities are
`Density::name`'s; zoom is a scale factor (`1`, `1.5`, `2`).

## Add a story in five lines

```sh
mkdir -p gallery/stories/controls/my-kind
printf 'id = "my-kind"\ntitle = "My kind"\nfamily = "controls"\nkinds = ["my-kind"]\nsize = [320, 80]\n\n[[variant]]\nid = "basic"\ntitle = "Basic"\nfile = "basic.svelte"\n' > gallery/stories/controls/my-kind/story.toml
printf '<script>\n  let { disabled = false } = $props();\n</script>\n\n<box padding={12}>\n  <my-kind anchor="subject" enabled={!disabled} />\n</box>\n' > gallery/stories/controls/my-kind/basic.svelte
cargo run -p skelter-gallery -- check
cargo run -p skelter-gallery -- sheet my-kind -o target/gallery/my-kind.png
```

A host with kinds of its own (cadatomic's `cadatomic-gallery`) keeps its stories in its own root and passes it
in `Extensions::story_roots`; the format is the same.
