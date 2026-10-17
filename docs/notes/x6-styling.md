# X6 styling: working notes

Lane X6, branch `worktree-agent-a9423c9b161b6f8f8`. Plan: `plans/milestones/X6-styling.md`. Research: `styling-gtk-css-qss.md`.

## Facts found while building

- Svelte's custom-renderer runtime funnels `class`, `class:` and `style:` through `renderer.setAttribute(el, 'class'|'style', string)` (`class_list_toggle`, `style_set_property` read the attribute back and rewrite the whole string). `style:a style:b` therefore arrives as several `setAttribute('style', ...)` calls in one job; the renderer coalesces them in a microtask (`markStyleDirty`).
- vite-plugin-svelte's virtual CSS module id ends in `.css`, and contains `.svelte?`. A replacement module id must avoid both (Vite's CSS pipeline would take `*.css`; vite-plugin-svelte's compile filter would take `.svelte`). The plugin maps each such id to `\0svelters-style:<n>.js`.
- Svelte prunes unknown-at-rule contents as unused selectors too (`@native gtk { entry > text {} }` becomes a comment), so `@native` blocks are extracted in a `style` preprocessor before the compile. Consequence: `@native` content is **not scoped** to the component; it is global to the toolkit.
- `kinds.generated.js`: 108 tags; 21 are ambiguous in at least one toolkit (shared CSS name such as `button` for button/togglebutton/linkbutton/lockbutton, shared Qt class such as `QWidget` for box/grid).
- GTK: the Adwaita theme paints buttons and entries with a `background-image` gradient, so `background-color` alone does not show (found by the host end-to-end test). The translator adds `background-image: none` next to `background-color` (an `info` diagnostic). `background:` shorthand needs nothing.
- GTK: `gtk_widget_get_color` is not in gtk4-rs 0.11 without the `v4_10` feature; tests use `style_context().color()` (deprecated, same value). `CssProvider::set_prefers_color_scheme` needs `v4_20`; the backend sets the property by GType name (`GtkInterfaceColorScheme`) instead, so the workspace feature set is unchanged.
- GTK: `GtkWidget` opacity reads back quantised to 8 bits (0.25 -> 0.251).
- GTK dark: `gtk-application-prefer-dark-theme` plus the per-provider `prefers-color-scheme` property is enough for both our `@media` rules and the theme's own widgets (verified by pixels in the dark gallery page).
- Qt: `QMainWindow` with a margin-ed child shows the window background only in the margin; tests sample inside the margin. The first pixel of a rounded button is anti-aliased (sample away from the corner).
- Qt: QSS `color` on a QLabel does reach `palette().windowText()`, so tests can read it without a pixel.
- Qt backend answers `SVL_STYLE_TARGET` 2 (QSS); the log backend and TestHost answer 1 (CSS). The Qt op-log replay therefore skips `sheet` and `inline` lines of a CSS log and replays classes, names, opacity and scheme.
- Hot reload: a new bundle context sends `SHEET` with null first, which clears sheets registered by the previous bundle, then registers its own under the same ids.

## Decisions that differ from the research note

- JS renders the target dialect (asks the backend once) instead of the backend translating: one implementation, golden-testable in Node, native code stays small. Cost: the app bundle carries both dialects of every sheet (about 2x the CSS text).
- Tokens have literal defaults on both toolkits (no `@accent_color` / `QPalette::Accent`), so light and dark look the same everywhere and tests are deterministic. System accent following is an open item; it needs the libadwaita decision (research 6.2 item 8).
- Named parts (`scale::slider`) are not in the vocabulary yet; `@native` covers them.
- `@native` content is unscoped (Svelte would prune and scope it), unlike the research sketch which kept it scoped.
- No `config` files for app-wide native sheets (`style: { gtk: 'theme.gtk.css' }`): not needed by the acceptance list.

## Merge notes for other lanes

- New element tags (X4 menus, X7 toplevels) must be added to `packages/style/src/kinds.generated.js`: run `xvfb-run -a python3 tools/metadata/style/gen_kinds.py`. `tools/metadata/style/test_gen_kinds.py` (part of `pnpm test:meta`) fails when a tag is missing. Until then, `<newtag>` type selectors report "unknown element" and nothing else breaks.
- `svl_backend` grew one trailing member (`style`). Backends constructing the struct by hand (tests with fake backends) need `style: None`.
- `backend.cpp.in`: the styling section sits before `b_request_frame`; the vtable initialiser gained `b_style` at the end. If X4/X7 append more members, keep the order of the header.
