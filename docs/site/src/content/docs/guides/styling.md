---
title: Styling
description: "The planned styling model, a portable vocabulary translated to GTK CSS and Qt stylesheets, with native escape hatches and theme tokens."
---

:::caution[Planned]
Styling is not implemented. Today the renderer drops `class`, `style` and `id` (`JS_ONLY_ATTRS` in `packages/renderer/src/index.js`), and `<style>` blocks compile but are not applied to native widgets. This page is the research outcome (2026-10-17) that feeds the "Later: Styling" row of `PLAN.md`. It is a design, not a plan with a date.
:::

## Four-layer model

```text
(d) widget props      layout & geometry: spacing, margins, halign/valign, hexpand, width-request,
                      xalign, wrap, ellipsize. Never CSS.
(a) portable style    <style> + class/class: + style/style:, validated against a small vocabulary,
                      translated per backend (GTK CSS / QSS / egui Style / WinUI XAML resources / AppKit)
(b) native escape     @native gtk { ... } / @native qt { ... } inside <style>, or app.gtk.css / app.qss in
                      config: passed through for that backend only, still scoped by Svelte, unvalidated
(c) theme             token set (--sv-*) -> GTK named colours/libadwaita vars, QPalette, egui Visuals,
                      WinUI ThemeResources, NSColor system colours; dark mode + accent come from here
```

Layout stays out of CSS. GTK has no layout properties in CSS, Qt's are per widget and fight layouts, and egui and WinUI have no CSS. In the counter experiment the same component looked different mainly because `QVBoxLayout` distributes extra height while `GtkBox` does not, and no CSS can fix that.

## Portable vocabulary

Deliberately small, limited to what egui, WinUI and AppKit can also honour per widget:

- `color`, `background-color` (solid colours and `var(--sv-*)` only)
- `font-family`, `font-size`, `font-weight`, `font-style`
- `padding` (on widgets with a visible box)
- `border-width`, `border-color`, `border-radius`
- `opacity`
- `min-width`, `min-height`

Selectors: type (svelters element names), class, id, descendant and child, and `:hover :active :focus :disabled :checked`. Media: `prefers-color-scheme`, `prefers-contrast`, `prefers-reduced-motion`. Named parts (`scale::slider`, `checkbutton::indicator`) map to GTK subnodes and Qt sub-controls through metadata. Transitions are best effort: native on GTK, dropped with a warning on Qt.

## Native escape hatch

`@native gtk { ... }` and `@native qt { ... }` at-rules pass through for one backend. Svelte keeps and scopes unknown block at-rules (verified with `@backend gtk { ... }`). Internal nodes and Qt class names inside them need `:global()`, because Svelte prunes selectors it cannot match to its own element tags. For app-wide themes, config files: `style: { gtk: 'theme.gtk.css', qt: 'theme.qss' }`.

## Svelte scoped CSS mapping

- `css: 'injected'` is a compile error with a custom renderer. `css: 'external'` works, and `compile().css.code` is the scoped sheet plus a source map.
- Scoping rides in `class`: the compiler appends the hash (`card svelte-5iljjr`). The renderer only has to forward `class`.
- `svelte/compiler`'s `parseCss()` returns the selector AST, so a translator is an AST walk with no third-party CSS parser.
- Svelte emits `:where(.svelte-h)` for later compounds. GTK rejects the whole rule, so the translator flattens it.

Translation table:

| Svelte output | GTK CSS | QSS |
| --- | --- | --- |
| `.card.svelte-h` | same (style classes) | same (dynamic property `class`) |
| `button.primary:where(.svelte-h)` | `button.primary.svelte-h` | `.QPushButton.primary.svelte-h` |
| `:hover :focus :disabled :checked` | same | same; `:active` becomes `:pressed`, `:not(:x)` becomes `:!x` |
| `[attr="v"]` | error | `[attr="v"]` on a dynamic or Q_PROPERTY |
| `:first-child`, `+`, `~` | native | error (never matches) |
| `:has`, `:is()`, `@media (min-width)` | error | error |
| `@media (prefers-color-scheme: dark)` | passed through | a separate dark sheet the host appends |
| `@keyframes`, `transition` | passed through | dropped with a warning |
| `var(--tok)` | passed through | `${--tok}` placeholder filled by the host |

An unsupported selector drops the whole rule (error). An unsupported property drops that declaration (warning, error in `strict`). For Qt this is a correctness requirement, not polish: the QSS parser stops at the first construct it cannot parse and drops every following rule.

## GTK CSS

GTK CSS is a real appearance engine: cascade, inheritance, `var()`, `@media prefers-color-scheme`, transitions, keyframes. It has no layout (`width`, `height`, `display`, flex, `gap`, `position`, `overflow` are rejected), no attribute selectors, and no `:is()`, `:where()` or `:has()`.

- Type selectors match the widget's CSS name, not the GType. Of 93 kinds with an element tag, 60 have tag equal to CSS name and 33 differ. Twelve CSS names are shared (`button` covers button, togglebutton, linkbutton and lockbutton). The renderer would add an `sv-k-<tag>` class only to kinds flagged `css.ambiguous`.
- `class` maps to `gtk_widget_set_css_classes`: cheap and local.
- Providers are display-wide. Adding, removing or reloading one restyles everything (about 240 to 260 ms for 1000 buttons here), so use them for HMR and theme switches only.
- The per-widget `GtkStyleContext` provider is deprecated since 4.10 and removed in GTK 5.
- GTK 4.20+ evaluates `prefers-color-scheme` per provider, so the host must set that property on every provider it creates.
- libadwaita variables (`--accent-bg-color`, ...) do not exist in plain GTK. Adopting `AdwApplication` would supply them; whether to adopt libadwaita is an open decision.

## Qt QSS

QSS is CSS syntax with brittle semantics: no variables, no media queries, fail-stop parsing, and the nearest sheet wins regardless of specificity.

- **Parse failure.** Unknown properties are skipped; unknown syntax (`--x: red`, `@media`, `:not()`, `:where()`, `&`) stops parsing, dropping every later rule. Unparseable values clobber earlier valid declarations.
- **Box trap.** `QPushButton { background: red; }` does nothing under Fusion until the box is customised (add `border: none`). Customising a box also loses native drawing for that widget.
- **The `class` trap.** QSS implements `.QPushButton` through a pseudo-property named `class`. Setting a dynamic property named `class` shadows it. The recommended fix: the renderer sets `class = "<QtClassName> <user classes>"`, for example `QPushButton primary svelte-h`.
- **Dynamic properties need a re-polish** (`unpolish`, `polish`, `update`) before they restyle.
- Set the app stylesheet before widgets are shown: 36 ms versus about 900 ms afterwards for 1000 buttons.
- Plan: Fusion plus `QPalette` by default, QSS only from the translator.

## Tokens and theming

One portable token set, `--sv-*`, is defined by the host per backend and per scheme.

| Token | GTK (plain theme) | GTK + libadwaita | Qt |
| --- | --- | --- | --- |
| `--sv-accent` | `@accent_color` | `var(--accent-bg-color)` | `QPalette::Accent` (6.6+) |
| `--sv-accent-fg` | `white` | `var(--accent-fg-color)` | `QPalette::HighlightedText` |
| `--sv-fg` / `--sv-bg` | `@theme_fg_color` / `@theme_bg_color` | `var(--window-fg-color)` / `var(--window-bg-color)` | `WindowText` / `Window` |
| `--sv-surface` | `@theme_base_color` | `var(--view-bg-color)` | `Base` |
| `--sv-border` | `@borders` | `var(--border-color)` | `Mid` |
| `--sv-radius`, `--sv-space-*`, `--sv-font-*` | literals | literals | literals |

On Qt without a platform theme, `setColorScheme(Dark)` was a no-op under Xvfb, so the host supplies the dark palette itself.

## `style` and `class`

`class=` and `class:` arrive as one flattened string. GTK sets the class list; Qt sets the `class` property and re-polishes (about 0.25 ms per widget).

`style=` and `style:` arrive flattened too, and neither toolkit has inline style. The recommendation is a structured channel: parse in JS, validate against the portable vocabulary, send structured setters to the host. GTK applies them as deduplicated style classes (an append-only provider flushed once per frame), Qt as a widget sheet wrapped in the exact class or as `QPalette`/`QFont` setters. Raw toolkit CSS in `style=` is not supported.

## Measured costs (1000 buttons, software rendering)

| Toolkit | Operation | Cost |
| --- | --- | --- |
| GTK | toggle a shared class on all 1000 | about 5 ms apply |
| GTK | provider load, add or remove | about 240 to 260 ms (global restyle) |
| GTK | one provider per inline style | 1751 ms apply (quadratic) |
| Qt | class toggled on all 1000 with repolish | 246 ms |
| Qt | app sheet reload (HMR) | about 0.7 s apply plus 0.5 s frame |
| Qt | per-widget `setStyleSheet` | about 0.3 ms per widget |

## Work items

1. Renderer: stop dropping `class`, `style`, `id`; add `set_classes` and `set_style` (or reserved `setProp` names), decided in `backend-abi.md`.
2. Vite plugin: intercept the style module, run the per-backend translators, surface diagnostics.
3. Translator package `@svelters/style`.
4. GTK backend: provider per module, token provider, inline-style dedupe store.
5. Qt backend: module to template map, token substitution, `class` property plus re-polish.
6. Define the `--sv-*` tokens and an override API.
7. Screenshot golden tests per backend.
8. Decide on libadwaita.

Sources: `docs/notes/styling-gtk-css-qss.md`, `PLAN.md`, `packages/renderer/src/index.js`.
