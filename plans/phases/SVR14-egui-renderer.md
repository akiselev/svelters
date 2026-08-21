# SVR14 - egui renderer and immediate-mode adaptation

References: `plans/architecture/native-api-ir.md`, `plans/architecture/runtime-stack.md`.

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

## Scheduling

Use egui/winit/eframe repaint mechanisms so an idle app remains idle. Svelte graph changes request repaint. Timers/async completions wake the UI through the host scheduler.

## HMR tests

Edit a live Svelte component while a TextEdit is focused, a ScrollArea is moved, and controls hold interaction state. Preserve state where NodeIdentity survives; document intentional resets when template identity changes.

## Acceptance

The portable reference app runs under egui from the same Svelte source. Ordinary UI edits cause no Rust rebuild. The event queue has no JS/egui reentrancy violations under stress. Idle CPU/repaint behavior and HMR state-preservation metrics are recorded.
