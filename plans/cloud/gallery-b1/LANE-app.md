# Lane app: the interactive gallery (GAL3)

Branch `ci/gb1-app`; record `plans/milestones/GAL3-gallery-app.md`. Read `BRIEF.md` first. You own
`crates/skelter-gallery-app/` (new; a binary `skelter-gallery-app` and a small lib so a host such as
cadatomic-gallery can start it with its `Extensions`) and its rows in the root `Cargo.toml`/`Cargo.lock`
(including the quire git dependencies and any `[patch]` lines). You build on `skelter-gallery` (core lane): merge
`origin/ci/gb1-core` into your branch as its API lands; never edit its files, and put what you need from it in
your record as a request.

1. **quire first (it can block you):** add `skelter_quire` (and `quire-core`, `quire-syntax` as it needs)
   from github.com/akiselev/quire (public) at a fixed rev. `skelter_quire` names svelters' crates by git; add a
   `[patch."https://github.com/akiselev/svelters"]` in the root manifest that points each at this workspace's
   path, as cadatomic's root `Cargo.toml` does (read it: `add_repo` `akiselev/cadatomic`, read only). Then
   `cargo check -p skelter-gallery-app`. If `skelter_quire`'s rev doesn't build against this svelters master,
   record exactly what breaks and the newest quire rev that does, and use a plain `text-edit` source pane only
   until the coordinator answers; don't edit quire.
2. **The window:** a sidebar (families → stories → variants, a filter box, coverage gaps marked), a canvas
   showing the selected story's grid (variants × states, in the chosen themes, zoom and density, each cell a
   separately themed root so themes sit side by side), a controls bar (themes, states, density, zoom, args), and
   the source pane showing the selected variant's file. Write the chrome in Svelte if the embed lets each
   preview cell be its own root with its own theme; otherwise write it in Rust on skelter; record which and why.
   The gallery's own chrome must not take the stories' theme.
3. **Live edit:** typing in the source pane previews through the provider's overlay (no save); Ctrl+S saves to
   disk; a change on disk (an agent's edit, or `base.css`) reloads through the existing watcher. A compile error
   shows in the cell with file:line and the source pane marks the line; the last good render stays.
4. **Inspect:** clicking a node in a cell shows what `skelter-gallery inspect` returns for it (the same JSON,
   presented), and a "copy command" action puts the matching `shot`/`inspect` command line on the clipboard, so
   a human can hand an agent the exact cell.
5. **Docs:** a section in `docs/skelter/gallery.md` (core writes the file; add your section after merging core),
   with how to run it: `cargo run -p skelter-gallery-app` and under `scripts`/Xvfb in a container.

Tests (skelter-test, headless): the sidebar lists the fixture stories; selecting one mounts its cells; an overlay
edit changes the rendered cell without touching disk; a save writes the file; a compile error keeps the last good
render and shows the error. Screenshots: send the user none yourself (lanes don't talk to the user); put two PNG
paths from your container in the record and describe them, and don't commit them.

Refuse rather than guess: a quire API you'd have to change, a hot-reload behaviour the watcher doesn't support
(record it as a request against svelters-modules with the smallest change).
