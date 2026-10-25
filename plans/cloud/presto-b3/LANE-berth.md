# Lane BERTH: presto_berth (berth's dock renderer on presto)

Repository: **berth** (github.com/akiselev/berth), branch `claude/presto-berth` from its default branch; push only
there. berth's own rules (README, HANDOFF, `check.sh`) apply: no `unsafe`, egui pinned exactly for egui_berth.
New crate `crates/presto_berth` beside `egui_berth`, depending on `berth-core` (path) and on svelters' presto
crates by git (`https://github.com/akiselev/svelters`, branch `ci/presto-base`; `presto-core`, `presto-widgets`,
`presto-text`, `presto-base`). Read svelters `plans/research/presto/architecture.md` §14 (windows, overlays,
tear-off) and `notes/round2/cadatomic-inventory.md` §1 (egui_berth's API and how it draws), then egui_berth's
source. Deliver: a `Dock` renderer that draws berth-core's tree with presto kinds and display lists (tabs, tab
strips, splitters with native drag, drop-zone previews in the window's overlay layer, floating panes as
layers, tear-off into new presto windows with global pointer capture), the same public seams egui_berth has
(`PaneHost` with a presto `PaneCx` instead of `egui::Ui`, `ChromePainter`, styles/text, `DockFrame::neighbour`),
author ids for accessibility, and a pane slot that can host an egui island (presto-egui-island) for
cadatomic's unported panes. Minimal building (user's instruction): `cargo check -p presto_berth`; write tests,
don't run them. Record: `docs/PRESTO-BERTH.md` in berth (landed, not run, questions). Report to the
coordinator `session_01CqZ417NuvkRujZaYSm7Pwo` with send_message when done.
