# SVR13 - portable widget semantics and structural policies

References: `plans/architecture/native-api-ir.md`.

## Goal

Define a useful portable native UI vocabulary without reducing GTK/Qt to a lowest common denominator or hiding backend-native capabilities.

## Portable API

Create `@svelters/portable` with semantic components such as Window, Row, Column, Grid, Label/Text, Button, TextInput, Checkbox, Radio, Slider, Select, Scroll, Split, Tabs, Menu/Action where semantics can be made consistent.

Portable components compile/render through backend semantic adapters; they are not fake HTML tags and do not imply browser layout/CSS.

Define portable event names, focus/disabled/visibility semantics, keyboard shortcuts, accessibility labels/roles where feasible, and explicit unsupported/degraded behavior.

## Structural policy

Finish the IR's child policies for single-child containers, layout-managed containers, indexed containers, named slots, model-driven widgets and custom adapters.

For native APIs, expose backend packages alongside portable components. An application may freely choose portable core plus Qt/GTK-specific widgets.

## Reference app

Grow the shared editor-shell fixture to include menu bar, toolbar/actions, split panes, form controls, scrolling, dialog invocation, status bar and keyboard shortcuts. Keep backend-specific features in isolated components so portable coverage is measurable.

## Acceptance

The same portable application source runs on GTK and QtWidgets with documented semantic parity. Backend-native components can coexist in the same source without changing the portable graph/runtime contracts. Unsupported portable behavior fails with a clear capability diagnostic rather than silently doing nothing.
