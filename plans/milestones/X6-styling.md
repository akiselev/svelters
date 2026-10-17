# X6: styling

Goal: Svelte `class`, `class:`, `style:`, `style=` and component `<style>` blocks style native GTK4 and Qt6 widgets. Research and the four-layer model: `docs/notes/styling-gtk-css-qss.md`. Running notes: `docs/notes/x6-styling.md`. ABI: `plans/architecture/backend-abi.md#abi-v23-additive-x6-styling`. User guide: `docs/site` styling page.

## Decisions

1. **One translator, two dialects.** `@svelters/style` (`packages/style`) translates the Svelte-scoped stylesheet (`css: 'external'`) at build time into GTK CSS and QSS and reports everything it drops as a diagnostic that names the toolkit. QSS output only ever uses constructs QSS parses (a parse failure drops every later rule).
2. **Style is data in the bundle.** `packages/build/style-plugin.js` replaces the virtual `?svelte&type=style&lang.css` module with `registerStyles(id, {gtk, qt, qtDark})` (JS). No `<style>` tag, no CSS pipeline. `@native` blocks are lifted out in a Svelte preprocessor (otherwise Svelte scopes them and prunes the toolkit's own selectors) and appended verbatim to their toolkit's sheet.
3. **One ABI op.** `svl_backend.style(op, handle, string)` carries classes, widget name, inline declarations, opacity, component sheets and the colour scheme. The first op (`TARGET`) tells JS which dialect to render, so only one dialect crosses the ABI and the native side stays small.
4. **Classes are the primitive.** GTK: `add/remove_css_class` diffed against what we set (never `set_css_classes`, which would drop `text-button`, `toggle`...). Qt: dynamic property `class = "<QtClass> a b"` plus `unpolish/polish`. Type selectors in `<style>` use our element names, mapped by a generated kinds table (`tools/metadata/style/gen_kinds.py`: GTK CSS name from a live probe, Qt class from the element overrides). Kinds that share a CSS name or Qt class get a `sv-k-<tag>` marker class, added by the renderer once any sheet is registered.
5. **Inline style is a structured channel.** The renderer parses `style` (which carries `style:` directives too), validates against the vocabulary, translates for the target, and sends declaration lines. GTK dedupes by content into shared classes (one append-only provider, flushed once per loop turn); Qt uses a per-widget sheet wrapped in the widget's class. `opacity` goes to the widget API.
6. **Tokens.** `--sv-*` names with built-in light/dark sets in JS (`tokens.js`). GTK: a `:root { --sv-x: v }` provider at lower priority than component providers, so a token change rewrites one small provider and `var()` resolves natively. Qt: component rules stay templates with `${--sv-x}`; JS renders them with the active set, the backend concatenates sheets into one application stylesheet (once per loop turn) and builds the `QPalette` from the same tokens. API: `setTokens`, `getTokens`, `setColorScheme`, `getColorScheme` from `@svelters/renderer/style`.
7. **Layout stays props.** Spacing, homogeneous, expand, alignment, size requests, wrapping, ellipsizing are widget props, never CSS (`width`, `height`, `display`, `gap`, `text-align`... are refused with a pointer to the prop). `margin` and `padding` are translated (they exist in both CSS dialects) but `margin` is documented as "prefer the margin props".

## Steps

1. Plan, ABI section, notes (this file, `backend-abi.md`).
2. `@svelters/style`: vocabulary and support matrix, kinds table, declaration and sheet translator, tokens, golden tests.
3. ABI member + host + log backend + TestHost; renderer `class`/`style`/`id`; build plugin; `fixtures/styles` op log.
4. GTK backend.
5. Qt backend (generator and templates only).
6. Integration tests (computed colours, class toggling, tokens, scheme), gallery styled page + themed variant, screenshots.
7. Docs site page, support matrix, final gates.

## Acceptance

- Unit: translator goldens, diagnostics snapshot, token rendering, renderer op tests.
- Integration on GTK and Qt: native read-back of computed colours (GTK `gtk_widget_get_color` / style context; Qt palette and `grab()` pixels), class toggling, inline style, runtime token change, scheme switch.
- Gallery: a styled page and its themed (dark) variant on each toolkit with screenshot tests.
- Gates: `pnpm test`, `pnpm docs:test`, `cargo-slot xvfb-run -a cargo nextest run --workspace`.
