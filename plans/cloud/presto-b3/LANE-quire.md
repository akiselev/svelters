# Lane QUIRE: presto_quire (quire's editor renderer on presto)

Repository: **quire** (github.com/akiselev/quire), branch `claude/presto-quire` from its default branch; push
only there. quire's rules (its CLAUDE.md/README) apply. New crate `crates/presto_quire` beside `egui_quire`,
depending on `quire-core`, `quire-text`, `quire-syntax` (path) and on svelters' presto crates by git
(`https://github.com/akiselev/svelters`, branch `ci/presto-base`). Read svelters
`plans/research/presto/architecture.md` §9 (text; the editor path) and §11.3 (TextInputClient),
`notes/round2/text-stack.md` §5 (editor strategy) and `notes/round2/cadatomic-inventory.md` §2 (egui_quire). Deliver:
the editor view on presto: per-line layout with presto-text (`parley_engine` per logical line or presto-text's
TextSystem; quire's ASCII monospace fast path kept), a two-generation line cache keyed as the text-stack
note says, only the viewport plus overscan shaped, wrapped rows from quire-text's `VisualLines`, a scroll
layer (scrolling is a compositor offset), caret (blinking as a compositor fast commit) and selections,
gutters, decorations (diagnostics squiggles as wavy lines), hover/completion popups in the overlay layer,
`TextInputClient` for IME (UTF-16 offsets: add them to quire-text if missing), the same host events egui_quire
emits. Minimal building (user's instruction): `cargo check -p presto_quire`; write tests, don't run them.
Record: `docs/PRESTO-QUIRE.md` in quire. Report to the coordinator `session_01CqZ417NuvkRujZaYSm7Pwo` with
send_message when done.
