# X6 styling: working notes

Lane X6, branch `worktree-agent-a9423c9b161b6f8f8`. Plan: `plans/milestones/X6-styling.md`. Research: `styling-gtk-css-qss.md`.

## Facts found while building

- Svelte's custom-renderer runtime funnels `class`, `class:` and `style:` through `renderer.setAttribute(el, 'class'|'style', string)` (`class_list_toggle`, `style_set_property` read the attribute back and rewrite the whole string). `style:a style:b` therefore arrives as several `setAttribute('style', ...)` calls in one job; the renderer coalesces them in a microtask (`markStyleDirty`).
- vite-plugin-svelte's virtual CSS module id ends in `.css`, and contains `.svelte?`. A replacement module id must avoid both (Vite's CSS pipeline would take `*.css`; vite-plugin-svelte's compile filter would take `.svelte`). The plugin maps each such id to `\0svelters-style:<n>.js`.
- Svelte prunes unknown-at-rule contents as unused selectors too (`@native gtk { entry > text {} }` becomes a comment), so `@native` blocks are extracted in a `style` preprocessor before the compile. Consequence: `@native` content is **not scoped** to the component; it is global to the toolkit.
- `kinds.generated.js`: 108 tags; 21 are ambiguous in at least one toolkit (shared CSS name such as `button` for button/togglebutton/linkbutton/lockbutton, shared Qt class such as `QWidget` for box/grid).
