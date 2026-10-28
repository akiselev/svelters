# Lane register: every widget usable from Svelte (POL2-REGISTER)

Branch `ci/pb2-register`; record `plans/milestones/POL2-REGISTER.md`.

Today `svelters_skelter::kinds::register_hosted` registers tier 0, scroll-area, collapsing-header, table, tree-view,
virtual-list, select and the menu kinds. skelter-widgets also has split, panel, popup, tooltip, toast, status bar,
modal, inner window, dialog buttons, menu bar (`views/`), tabs (`tabs/kinds.rs`) and the colour/date pickers and
calendar (`pickers/kinds.rs`), which no Svelte code can mount (GAL2 request 1, POL1-VIEWS Q4).

1. **Register them.** Call `views::register_all` (minus what is registered already), `tabs::kinds::register` and
   `pickers::kinds::register` from `register_hosted`; add their surface rows (tags, props, events, slots) to
   `surface/tier1.rs` from the kinds' own specs, never invented; regenerate `metadata/semantics/skelter.json`, the
   typings and the docs reference with their generators. Tag names follow the existing surface (kebab-case, the
   egui surface's names where one exists). A kind whose surface needs a decision (a popup's anchoring, a modal's
   ownership, how tabs take their pages) gets the smallest shape that matches svelters-egui's surface for the same
   element; record the choice.
2. **Open states.** `context-menu` gets an `open` prop as `combo-box` has (GAL2 request 2); tooltips and toasts are
   showable for a shot: a gallery state (`open`) or a prop, plus the virtual clock stepped past the hover delay, so a
   story shows them without real input. Tests for each.
3. **Stories** in `gallery/stories/` for every newly mountable kind, in the GAL2 style (variants a polisher needs:
   sizes, long text, RTL, overflow, inside a dense layout), and `context-menu`'s story open. `skelter-gallery check`
   passes; `list` shows no registered kind without a story.
4. **Polish them** (the polish-b1 loop): render each new family's sheets in the four themes at zoom 1 and 2, compare
   with the VS Code reference look (DESIGN.md §6), fix what is off in the kinds' look (`views/`, `tabs/`, `pickers/`,
   their `base.css` blocks), record the rest with the `shot` command that shows it.

Done: every skelter-widgets kind is mountable from Svelte with a story; open states render; findings fixed or
recorded; the six workflows green. Refuse rather than guess: a prop the kind's spec doesn't have, a surface choice
that would contradict svelters-egui's surface (record it with a recommendation).
