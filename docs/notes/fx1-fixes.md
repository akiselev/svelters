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

## fx1-gen (renderer side)

- `gen_bindings.py` reads AppKit (`appkit_bindings`: overrides + `metadata/appkit`) and WinUI (`winui_bindings`: overrides + `metadata/winui`)
  bindings, validated against the facts (property in the class chain and writable, signal in the chain's overrides, arg0 needs a payload/args).
  A binding on a base class applies to every element class deriving from it. Enum refs (`enum<...>`) are renderer type `string`.
- Merge order gtk, qt, appkit, winui. Same renderer-visible fields: one descriptor (`toolkits` lists them). Different (slider `value`: Qt
  `valueChanged`, WinUI `value-changed`): `BINDINGS[tag][name]` is an array of descriptors; `bind()` picks the first whose prop and signal
  the host resolves (`pickVariant`), else the first. Generated tag/prop keys with hyphens are quoted.
- `TEXT_PROPS` is generated (`text_props()`, union of `kinds.<id>.text_content` over the semantics files that exist); rerun gen_bindings
  when a semantics file changes.
- Not done: per-binding Rust tests in the AppKit/WinUI generators (`tests()`/`gen_tests()`); they would touch generated Rust that cannot
  run on Linux, so left out as disproportionate.
- Unverified on Mac/Windows: that a real host resolves `click`/`value-changed` through `signalId` (tests stub `signalId`).
