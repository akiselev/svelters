# Lane stories: a story for every skelter kind (GAL2)

Branch `ci/gb1-stories`; record `plans/milestones/GAL2-stories.md`. Read `BRIEF.md` first: the story format there
is your spec. You own `gallery/` (new). You don't own `crates/skelter-gallery*` (core and app lanes).

1. **The package and the format doc:** `gallery/package.json` (a svelters source package that
   `SourceConfig::for_path` accepts; check against `crates/svelters-modules/src/source/mod.rs` and an existing
   fixture such as `fixtures/gallery-common` or `examples/counter`), and `gallery/README.md`: the format, the
   axes, the `subject` anchor rule, how `disabled` works, and how to add a story in five lines.
2. **The inventory:** every kind the app runner registers (`svelters_skelter::kinds::register_builtin` and what
   it pulls in from `skelter_widgets::host`; the element names in `metadata/semantics/skelter.json`; the kinds in
   `crates/skelter-widgets/src/{controls,text,pickers,views,tabs,dialogs,table}`). Put it in your record as a
   table: kind, family, story id, notes. Kinds that are internal parts (not usable as elements) are listed and
   skipped with the reason.
3. **The stories,** family by family, pushing after each family: controls (button, toggle, checkbox, radio,
   switch, slider, progress, spinner, hyperlink, label and rich text, icon, image, separator, collapsing,
   frame), text (text-edit single and multi-line, password, number and quantity fields, placeholder, selection,
   error), pickers (colour, date, calendar), views (scroll, split, panel, overlay, popup, tooltip, toast,
   status bar, modal, inner window, dialog buttons), menus (menu bar, menus, context menu, select/combo),
   tabs, data (table, tree, virtual list), layout (box, stack, grid, sides). Variants cover what a polisher
   needs to see: each visual variant the kind supports (sizes, emphasis, icon and text combinations), long and
   empty text, RTL text where the kind lays text out, overflow, and the kind inside a dense layout. Use the
   kinds' real props (from `skelter.json` and the kinds' sources); never invent a prop. Text is plain English
   sample text; anchors are ids.
4. **Render them** once core's `shot` and `check` land (merge `origin/ci/gb1-core` when needed): run
   `skelter-gallery check` and fix every story it fails. Then render one sheet per family and look at each.
   Write what looks wrong in your record under "Polish findings", one row per issue: story/variant, state,
   theme, what is wrong, the `shot` command that shows it. Don't fix the widgets (BRIEF: stay out of existing
   crates); these rows are the next batch's input.

Done looks like: `check` passes on every story; the coverage list has no registered kind without a story or a
recorded reason; the findings table exists. Refuse rather than guess: a prop you can't find in a kind's source
(record it), a kind you can't mount from Svelte (record it with the error).
