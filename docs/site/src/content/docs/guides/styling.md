---
title: Styling
description: "Style native widgets from Svelte: class, class:, style:, scoped style blocks, --sv-* design tokens and dark mode, translated to GTK CSS and Qt stylesheets."
---

Svelte's `class`, `class:`, `style:` and `style` work on native elements, and a component's `<style>` block styles them. The build translates the stylesheet into GTK CSS and Qt stylesheets (QSS) and reports everything it had to drop, naming the toolkit. Colours and sizes can come from `--sv-*` tokens that switch with the colour scheme at runtime.

```svelte
<script>
  import { setColorScheme, setTokens } from '@svelters/renderer/style';
  let count = $state(0);
  let warn = $state(false);
</script>

<box class="card" orientation="vertical" spacing="8">
  <label class="count" class:warn style:font-weight={count > 2 ? 'bold' : null}>Count: {count}</label>
  <button class="primary" onclick={() => count++}>Increment</button>
  <button onclick={() => setColorScheme('dark')}>Dark</button>
  <button onclick={() => setTokens({ '--sv-accent': '#e5a50a' })}>Amber accent</button>
</box>

<style>
  .card { background-color: var(--sv-surface); border: 1px solid var(--sv-border); border-radius: var(--sv-radius); padding: var(--sv-space-3); }
  .count.warn { color: var(--sv-danger); }
  button.primary { background-color: var(--sv-accent); color: var(--sv-accent-fg); border-radius: 6px; padding: 6px 14px; }
  button.primary:hover { background-color: var(--sv-accent-hover); }
  @media (prefers-color-scheme: dark) { .card { border-color: #666; } }
</style>
```

The same component, light and dark, on both toolkits (the `styling` and `styling dark` pages of the widget galleries):

| | Light | Dark |
| --- | --- | --- |
| GTK 4 | ![GTK, light scheme](../../../assets/styling/gtk-light.png) | ![GTK, dark scheme with an amber accent token](../../../assets/styling/gtk-dark.png) |
| Qt 6 | ![Qt, light scheme](../../../assets/styling/qt-light.png) | ![Qt, dark scheme with an amber accent token](../../../assets/styling/qt-dark.png) |

The Qt window stretches the page vertically because `QVBoxLayout` distributes spare height; that is layout, which CSS never controls (see [Layout stays in props](#layout-stays-in-props)).

## Four layers

| Layer | What | Where |
| --- | --- | --- |
| Layout | spacing, alignment, expand, size requests, wrapping, ellipsizing | widget props, never CSS |
| Portable style | `<style>`, `class`, `class:`, `style:`, `style` limited to a defined vocabulary | translated to GTK CSS and QSS |
| Native escape | `@native gtk { }` and `@native qt { }` inside `<style>` | passed through verbatim to one toolkit |
| Tokens | `--sv-*` custom properties and the colour scheme | one token set mapped per toolkit |

## How it works

```text
<style> in App.svelte
  | Svelte compile (css: 'external'): scoped hash on selectors and on the elements' class
  | svelters:style Vite plugin -> @svelters/style translator
  v
registerStyles("src/App.svelte", { gtk, qt, qtDark })      (data in the bundle, no <style> tag)
  v
renderer: class / id / style attributes + sheets + tokens -> host.style(op, handle, text)   (ABI v2.3)
  v
GTK: one GtkCssProvider per component, widget css classes          Qt: one application stylesheet, class property
```

- **Scoping.** Svelte appends its hash class (`svelte-1n46o8q`) to every selector and every element that carries a class, so a component's rules only match its own elements. The renderer forwards the class string unchanged.
- **Type selectors use Svelters element names** (`button`, `label`, `entry`), not toolkit names. The translator maps them: `button` becomes the CSS node `button` on GTK and `.QPushButton` on Qt. Kinds that share a toolkit name with another kind (for instance `button`, `togglebutton` and `linkbutton` are all the CSS node `button`) get a marker class `sv-k-<tag>` from the renderer, and the selector uses it, so `button` never matches a toggle button. An element that does not exist on a toolkit (`togglebutton` on Qt) drops the rule there with a warning.
- **Classes are cheap and local.** GTK adds and removes only the classes the app set (the widget keeps its own `text-button` or `toggle`). Qt sets the dynamic property `class` to `"<QtClass> your classes"`, so `.QPushButton` keeps matching, and re-polishes that one widget.
- **`id`** sets the widget name (GTK) or object name (Qt), so `#id` selectors match.

## `style` and `style:`

Inline style is parsed in JavaScript, checked against the [support matrix](#support-matrix) and sent to the backend as declarations in its dialect. Raw toolkit CSS is not accepted in `style`; use `@native`.

- GTK deduplicates by content: each distinct declaration set becomes a shared class `sv-s<n>` in one append-only provider, flushed once per main-loop turn. Changing a widget to an already seen style is a class swap. Inline rules sit above component rules, as inline style does on the web.
- Qt gives the widget its own stylesheet, wrapped in the widget's exact class (a bare declaration list would leak to its descendants).
- `opacity` in `style` never goes through CSS: it is `gtk_widget_set_opacity` on GTK and a `QGraphicsOpacityEffect` on Qt, which is also why Qt supports `opacity` only inline.
- Several `style:` directives that change in one update arrive as one operation.

Continuously animated values (`style:opacity={t}` every frame) are better driven through props or `opacity`; every new distinct style string is a new GTK class, and a Qt widget sheet re-polishes its widget.

## Selectors

| Selector | GTK | Qt |
| --- | --- | --- |
| element name, `.class`, `#id`, `*` | yes | yes |
| descendant and child (`a b`, `a > b`) | yes | yes |
| `:hover`, `:focus`, `:disabled`, `:checked` | yes | yes |
| `:active` | yes | becomes `:pressed` |
| `:focus-visible` | yes | approximated by `:focus` (warning) |
| `:first-child`, `:last-child`, `:only-child`, `:focus-within`, `:backdrop`, `:selected` | yes | `:selected` only; the rest is an error |
| `:not(.x)` | yes | `:not(:state)` becomes `:!state`; otherwise an error |
| `+`, `~` | yes | error (QSS parses them but they never match) |
| `[attr]` selectors, `:is()`, `:has()`, `::part`, nesting `&` outside Svelte's output | error | error |
| `@media (prefers-color-scheme: dark)` | yes | yes (a dark sheet appended while the dark scheme is active) |
| `@media (prefers-contrast)`, `(prefers-reduced-motion)` | yes | error |
| `@keyframes` | yes | dropped with a warning |

An unsupported selector drops the whole rule and reports an error. Svelte's own `:where(.svelte-h)` on later compounds is flattened, because GTK rejects the whole rule otherwise.

## Support matrix

Properties the translator accepts, per toolkit. Everything else is dropped with a warning that names the toolkit and, for layout properties, the prop to use instead.

| Property | GTK | Qt | Notes |
| --- | --- | --- | --- |
| `color`, `background-color`, `border`, `border-width`, `border-style`, `border-color`, `border-radius`, `padding`, `font-family`, `font-size`, `font-weight`, `font-style`, `min-width`, `min-height`, `border-top`, `padding-top`, `border-top-width`, `border-top-style`, `border-top-color`, `border-right`, `padding-right`, `border-right-width`, `border-right-style`, `border-right-color`, `border-bottom`, `padding-bottom`, `border-bottom-width`, `border-bottom-style`, `border-bottom-color`, `border-left`, `padding-left`, `border-left-width`, `border-left-style`, `border-left-color`, `border-top-left-radius`, `border-top-right-radius`, `border-bottom-left-radius`, `border-bottom-right-radius` | yes | yes | |
| `background` | yes | yes | colour only on Qt: gradients are not translated |
| `margin`, `margin-top`, `margin-right`, `margin-bottom`, `margin-left` | yes | yes | layout: prefer widget props (margin-start, spacing, ...) |
| `opacity` | yes | inline | QSS has no opacity: Qt honours it for style= / style: only (QGraphicsOpacityEffect) |
| `text-decoration` | yes | no | QSS has text-decoration on some widgets only; not translated |
| `letter-spacing`, `box-shadow`, `text-shadow` | yes | no | |
| `transition`, `transition-property`, `transition-duration`, `transition-timing-function`, `transition-delay` | yes | no | QSS has no transitions |
| `animation`, `animation-name`, `animation-duration`, `animation-timing-function`, `animation-delay`, `animation-iteration-count`, `animation-direction`, `animation-fill-mode`, `animation-play-state` | yes | no | QSS has no animations |

`yes` is translated, `inline` means only in `style=` and `style:`, `no` is dropped with a warning.

Values are checked per toolkit. Qt is strict because QSS clobbers earlier declarations with a value it cannot parse: `oklch()`, `color-mix()`, `calc()`, `rem` units, percentages, gradients and `!important` are errors there. GTK rejects `!important` and percentage padding.

Two compensations are applied for you:

- **Qt boxes.** A QSS background on a button, combo box or spin box only paints once the box is customised, so `border: none` is added to a rule that sets a background but no border (not on labels, frames and windows).
- **GTK themes.** The theme paints buttons and entries with a `background-image` gradient that hides `background-color`, so `background-image: none` is added next to it.

### Layout stays in props

`width`, `height`, `max-width`, `max-height`, `display`, `position`, `flex`, `gap`, `text-align` and `cursor` are refused with a pointer to the right tool. GTK CSS has no layout properties, QSS's are per widget and fight layouts, and the other planned toolkits have no CSS at all.

| You want | Use |
| --- | --- |
| space between children | the container's `spacing` prop |
| children the same size | `homogeneous` |
| grow to fill | `hexpand`, `vexpand`, `halign`, `valign` (and per-child attributes such as `parent:column`) |
| fixed size | `width-request`, `height-request` (`min-width`, `min-height` in CSS only set a minimum) |
| text alignment, wrapping, ellipsis | `xalign`, `justify`, `wrap`, `ellipsize` |
| outer spacing | the margin props; CSS `margin` is accepted but is a layout property in disguise |

## Diagnostics

The Vite plugin prints one line per dropped construct, naming the toolkit, with the position inside the file:

```text
[plugin svelters:style] warning [qt] property 'box-shadow' is not supported on qt
[plugin svelters:style] warning [qt] <togglebutton> is not an element on qt; rule dropped
[plugin svelters:style] error [qt] value 'oklch(0.6 0.1 250)' for background-color uses 'oklch', which QSS cannot parse
[plugin svelters:style] error [qt] combinator '+' never matches in QSS; rule dropped
```

Nothing is dropped silently. Build all targets from one source and read the output of each toolkit. A component that is clean on every toolkit you ship prints nothing. Inline `style` problems are reported once per distinct message at runtime with `console.warn`.

Options (the third argument of `svelteConfig` in `vite.config.js`, or environment variables):

```js
export default defineConfig(svelteConfig(import.meta.dirname, undefined, {
  style: {
    strict: true,          // or SVELTERS_STYLE_STRICT=1: every warning and error fails the build
    verbose: false,        // or SVELTERS_STYLE_VERBOSE=1: also print the info lines (compensations)
    tokens: ['--sv-brand'] // token names you define with setTokens(), so they are not reported as unknown
  },
}));
```

## Tokens and the colour scheme

`--sv-*` tokens are the portable way to pick colours and sizes. The defaults come in a light and a dark set:

| Token | Light | Dark |
| --- | --- | --- |
| `--sv-bg` | `#f6f5f4` | `#242424` |
| `--sv-fg` | `#2e3436` | `#ffffff` |
| `--sv-surface` | `#ffffff` | `#303030` |
| `--sv-border` | `#cdc7c2` | `#4a4a4a` |
| `--sv-accent` | `#3584e4` | `#3584e4` |
| `--sv-accent-hover` | `#1c71d8` | `#4a90e8` |
| `--sv-accent-fg` | `#ffffff` | `#ffffff` |
| `--sv-muted` | `#77767b` | `#9a9996` |
| `--sv-danger` | `#c01c28` | `#ff7b63` |
| `--sv-radius` | `6px` | `6px` |
| `--sv-space-1` to `--sv-space-4` | `4px`, `8px`, `12px`, `16px` | same |
| `--sv-font-size`, `--sv-font-family` | `11pt`, `sans-serif` | same |

```js
import { setTokens, getTokens, setColorScheme, getColorScheme } from '@svelters/renderer/style';

setColorScheme('dark');                       // tokens, Qt palette, GTK prefers-color-scheme and dark sheets follow
setTokens({ '--sv-accent': '#e5a50a' });      // override app-wide
setTokens({ '--sv-accent': null });           // back to the scheme's default
```

Calls in the same job are batched, and a token set that did not change sends nothing.

- **GTK.** Tokens are a `:root { --sv-x: value; }` rule in one small provider below the component providers. Component CSS keeps `var(--sv-x)` and GTK resolves it, so a token change rewrites that one provider. The scheme is set as the `prefers-color-scheme` property of every provider the host owns (GTK evaluates `@media (prefers-color-scheme)` per provider) and as the toolkit's dark preference, so the theme's own widgets follow too.
- **Qt.** QSS has no variables. The translator keeps component rules as templates with `${--sv-x}` placeholders; the renderer fills them from the active token set and the backend concatenates the sheets into one application stylesheet. The same tokens become the application `QPalette` (`--sv-bg` is Window and Button, `--sv-fg` the text roles, `--sv-surface` is Base, `--sv-accent` and `--sv-accent-fg` are Highlight and HighlightedText), so widgets you did not style match. A token without a value and without a `var()` fallback removes its declaration rather than emitting something QSS would mis-parse. Qt sets the sheet at once while no window is visible (startup) and one zero-delay timer turn later after that, so a theme switch is one re-polish.
- Tokens are app-wide on both toolkits. A per-subtree override works on GTK only through `@native gtk`.
- `@media (prefers-color-scheme: dark)` in a component follows `setColorScheme`, not the operating system. Following the system setting is an open item.
- Dark mode on Qt under X11 without a platform theme is the palette above: `QStyleHints::setColorScheme` did not change it in testing, so Svelters does not rely on it.

## `@native`

`@native gtk { }` and `@native qt { }` inside `<style>` are passed to that toolkit only, as written, after the translated rules:

```svelte
<style>
  .field { color: var(--sv-fg); }

  @native gtk {
    entry > text { color: red; }
    button.text-button:hover { box-shadow: inset 0 0 0 1px @accent_color; }
  }

  @native qt {
    .QLineEdit { selection-background-color: ${--sv-accent}; }
    QPushButton::menu-indicator { image: none; }
  }
</style>
```

- Nothing in a native block is validated or scoped to the component: Svelte would prune the toolkit's own selectors (`entry > text`) as unused, so the blocks are lifted out before the compile. Use your own classes to limit them.
- Qt token placeholders (`${--sv-accent}`) are filled; everything else is verbatim, and QSS stops parsing at the first construct it does not understand, so a typo in a Qt block can disable every rule after it.
- GTK reports a native block's parse errors on stderr (`[svelters-gtk] css parse error`).
- Use native blocks for internal nodes (`entry > text`, `scale > trough > slider`), Qt sub-controls (`::indicator`, `::handle`), gradients, shadows and anything outside the vocabulary.

## Performance notes

- A class change costs a class change: GTK about 3 to 6 ms for 1000 buttons when no rule changes the look, Qt about 0.25 ms per widget for the re-polish.
- Providers and the Qt application stylesheet are the expensive path (about 240 ms on GTK and 0.7 s on Qt per 1000 widgets, software rendering). They are used for sheet registration, hot reload and token or scheme changes, never per interaction.
- The Qt sheet is applied before the first window is shown (36 ms against about 900 ms after, measured over 1000 buttons).
- Hot reload (`svelters-build --watch`) re-registers sheets under the same ids, which reloads one GTK provider or one Qt sheet.

Measurements: `docs/notes/styling-gtk-css-qss.md`.

## Limits and open items

- Parts (`scale::slider`, `checkbutton::indicator`) are not in the portable vocabulary yet; use `@native`.
- No libadwaita: tokens are literal values on both toolkits, so the system accent colour is not followed.
- The scheme is chosen by the app, not read from the system.
- Svelte's `transition:` and `animate:` stay compile errors; CSS `transition` works on GTK only.
- Unscoped `@native` content is global to its toolkit.

Sources: `docs/notes/styling-gtk-css-qss.md`, `docs/notes/x6-styling.md`, `plans/milestones/X6-styling.md`, `plans/architecture/backend-abi.md` (ABI v2.3).
