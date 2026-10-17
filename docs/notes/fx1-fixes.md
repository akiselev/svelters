# FX1: follow-up fixes after ST1 / MAC1 / WIN1 (lane notes)

Branched from master at ff0fcb4. Written as the work goes.

## Metadata shape (step 0)

- **Text children** (`<button>Save</button>`): one key per toolkit, in the override YAML, never in generated output.
  - GTK: top-level `text_prop: { name, provenance }` in `metadata/overrides/gtk/<Kind>.yaml` (Label, Button).
    Inherited along the kind chain (Button applies to ToggleButton/LinkButton, as the old rules did).
    `ingest.py` skips the key (not merged into `metadata/gtk`), `gen_semantics.py` reads it from the override.
  - Qt: `elements[].text_prop: text` (native Q_PROPERTY name; resolved to the JS name, so N1's rename does not touch it).
  - AppKit: `element.text_prop: title` (NSButton).
  - WinUI: `element.text_prop: Content` (renamed from WIN1's `text_content`; the generated Rust is unchanged).
  - The spec field stays `kinds.<k>.text_content` (the compiler already reads it); the `text_content` entries in
    `metadata/semantics/rules/{gtk,qt}.yaml` are gone.
- **Bindings** for AppKit/WinUI use the full GTK/Qt binding format (direction, type, change, echo, lifetime);
  MAC1's short form (`name/property/signal`) is replaced. Names are native kebab-case prop names.
  - AppKit: `button` `state` (action, read back), `slider` `double-value` (action, read back), `text-field`
    `string-value` (control-text-did-change arg0). No echo: AppKit never sends these for programmatic sets.
  - WinUI: `check-box` `is-checked` (`click`, read back: ToggleButton toggles before Click; programmatic sets raise
    only Checked/Unchecked), `slider` `value` (value-changed arg0, compare), `text-box` `text` (text-changed arg0, compare).
    Unverified on Windows: Click-after-toggle ordering, and the async TextChanged echo.
