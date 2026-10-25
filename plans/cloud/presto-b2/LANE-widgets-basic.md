# Lane WIDGETS-BASIC: presto-widgets (interaction substrate, paint helpers, theme, Tier 0)

Owns `crates/presto-widgets/src/{lib.rs,interact/**,paint/**,theme/**,controls/**}`. Port from svelters-egui
`interact/` (the 13 interaction units: keep egui's click semantics), `paint/`, `theme/` (tokens, not
`egui_style.rs`), `widgets/` and `elements/` for Tier 0 (EGUI.md §5): box, label (with rich runs), button,
toggle-button, checkbox, radio, switch, slider, progress-bar, spinner, separator, icon (icon kit SVGs via
resvg into an `ImageId`), image, hyperlink, frame, stack, sides, grid. Each kind implements `presto_core::Kind`
with a `KindSpec` (props with change classes, signals with passive/cancelable, a11y role and name sources).
Record into display lists with presto-base primitives; text through presto-text `TextLayout`. Tests (written,
not run): one per kind for spec, record output (display-list goldens), and interaction (press/click/toggle).
