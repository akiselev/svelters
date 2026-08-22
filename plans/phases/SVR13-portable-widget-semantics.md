# SVR13 - portable widget semantics and structural policies

References: `plans/architecture/native-api-ir.md` and `plans/architecture/semantic-graph.md`.

## Goal

Define a useful portable native UI vocabulary without reducing GTK/Qt to a lowest common denominator or hiding backend-native capabilities, and make every portable widget populate the canonical semantic/accessibility model established in SVR03.

## Portable API

Create `@svelters/portable` with semantic components such as Window, Row, Column, Grid, Label/Text, Button, TextInput, Checkbox, Radio, Slider, Select, Scroll, Split, Tabs, Menu/Action where behavior can be made consistent.

Portable components compile/render through backend semantic adapters; they are not fake HTML tags and do not imply browser layout/CSS.

For each portable component define:

- visual/interaction properties and event names;
- focus, disabled, visibility and keyboard-shortcut behavior;
- default canonical `SemanticRole`;
- accessible-name/value derivation rules;
- canonical states and supported actions;
- semantic relations such as labelled-by where applicable;
- explicit unsupported/degraded behavior.

Do not introduce another accessibility type system in this package. Portable widgets populate the SVR03 `Semantics` record, and the GTK/Qt/egui renderers project that record into their native accessibility systems.

## Structural policy

Finish the IR's child policies for single-child containers, layout-managed containers, indexed containers, named slots, model-driven widgets and custom adapters.

For native APIs, expose backend packages alongside portable components. An application may freely choose portable core plus Qt/GTK-specific widgets. Native widgets may provide backend metadata that derives canonical semantics, but native toolkit enums remain implementation details.

## Reference app

Grow the shared editor-shell fixture to include menu bar, toolbar/actions, split panes, form controls, scrolling, dialog invocation, status bar and keyboard shortcuts. Keep backend-specific features in isolated components so portable coverage is measurable.

Maintain a renderer-independent expected semantic snapshot for the reference app. GTK and Qt native accessibility mappings must be traceable back to those same canonical nodes.

## Acceptance

The same portable application source runs on GTK and QtWidgets with documented visual/interaction and semantic parity. Its TestHost semantic snapshot contains roles, names, relevant values/states/actions and relations for every interactive portable control. Backend-native components can coexist in the same source without changing the portable graph/runtime contracts. Unsupported portable behavior or semantic projection fails with a clear capability diagnostic rather than silently doing nothing.
