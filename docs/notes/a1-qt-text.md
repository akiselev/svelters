# a1 qt-text notes

- QTextEdit `html`: dropped (toHtml() returns a full document, never equals the sample). `markdown` verified with sample `x\n\n` (single-quoted YAML; the generator does not escape a real newline in samples).
- QTextBrowser `readOnly`/`undoRedoEnabled` have no direct accessors there (inherited from QTextEdit, not exposed through the textbrowser element): left out. `source` (QUrl) and QKeySequenceEdit `keySequence` convert via S1. QTextBrowser URL-arg signals (anchorClicked, sourceChanged) and QKeySequenceEdit keySequenceChanged not tried (no QUrl/QKeySequence signal arg type).
- Enum JS names are kebab-case (`password`, `rich-text`, `no-wrap`).
- plainText bindings use `value: prop`; textChanged() has no args. `document()` is annotated as a method on textedit (borrowed handle).
- blockCountChanged trigger must change the text on every $i (`"a\n".repeated($i+1)`), since the test fires it twice.
- The gallery window is small, the text page scrolls.
