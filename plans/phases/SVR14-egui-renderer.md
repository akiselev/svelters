# SVR14 - egui renderer and immediate-mode adaptation

References: `plans/architecture/native-api-ir.md`, `plans/architecture/runtime-stack.md`, `plans/architecture/ecosystem-integrations.md`.

## Goal

Use egui as the architectural stress test: Svelters retains the logical graph while the backend itself is immediate-mode. Gain hot-reloadable egui UI without recompiling Rust for ordinary Svelte changes.

## Render model

Each egui pass traverses a graph snapshot and emits egui calls. Native widget objects are not stored. Stable Svelters NodeIdentity maps to `egui::Id` for persistent interaction/scroll/focus memory.

Strict reentrancy rule:

```text
graph snapshot -> egui pass -> collect Response-derived events -> end Ui borrows -> dispatch JS -> graph mutations -> request repaint
```

Never invoke arbitrary QuickJS callbacks while an `egui::Ui` borrow is active.

## Initial coverage

Window/panels, Row/Column-like layouts, Label, Button, TextEdit, Checkbox/Radio, Slider/DragValue, ComboBox, ScrollArea, Grid, menu, separators, selectable values, and focus/keyboard handling needed by the reference editor shell.

Curate renderer operations rather than trying to dynamically expose arbitrary generic egui Rust methods. Raw/custom egui extensions land through the plugin SDK later.

## Embedding contract and egui-workbench alignment

The renderer must support both:

- a standalone eframe/winit host for a normal Svelters egui application; and
- rendering a Svelters graph root into a caller-owned `&mut egui::Ui`/host surface without taking ownership of the process event loop.

Do not depend on `egui-workbench` or reuse its application-shell protocol as the Svelters graph protocol. Align the generic concepts instead: stable `WidgetKey`/NodeIdentity-derived egui IDs, deferred event delivery, bounded plugin data, last-known-good behavior, and generation-aware hot reload. Generic dynamic-extension mechanics belong in Switchyard.

The embedding contract is a prerequisite for the SVR19 `svelters-egui-workbench` dogfood adapter.

## Scheduling

Use egui/winit/eframe repaint mechanisms so an idle app remains idle. Svelte graph changes request repaint. Timers/async completions wake the UI through the host scheduler.

## HMR tests

Edit a live Svelte component while a TextEdit is focused, a ScrollArea is moved, and controls hold interaction state. Preserve state where NodeIdentity survives; document intentional resets when template identity changes.

## Acceptance

- the portable reference app runs under standalone egui from the same Svelte source;
- the same renderer can render a component into a caller-owned egui `Ui` test surface;
- ordinary UI edits cause no Rust rebuild;
- the event queue has no JS/egui reentrancy violations under stress;
- idle CPU/repaint behavior and HMR state-preservation metrics are recorded.
