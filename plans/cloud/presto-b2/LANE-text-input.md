# Lane TEXT-INPUT: presto-widgets (text editing on quire-text)

Owns `crates/presto-widgets/src/text/**`. Build text-edit (single and multi-line, password, char limit,
placeholder), the value boxes (number/quantity spinners), the table cell editor and the colour hex field on
**quire-text** (github.com/akiselev/quire, crate `quire-text`, git dependency at the sha in cadatomic's
`pins.toml`: 3e78dcd3bf74abc2c81209f0109caff4d9193529), rendered with presto-text `TextLayout`. Implement
`presto_base::input::TextInputClient` (UTF-16 document offsets: add the ropey-based conversion in your crate if
quire-text lacks it, and record it as a request for quire), caret blink as a compositor fast commit,
selection, word movement per OS (quire-text's word rules), clipboard and primary selection through the
platform, IME preedit display, IME-first key routing (architecture §11.2). Tests (written, not run): editing,
selection, IME composition, UTF-16 mapping, the `egui_measure` pins 1–7 equivalents.
