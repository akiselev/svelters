# SVR17 - styling and native style escape hatches

## Goal

Make scoped Svelte styles useful for native applications without accidentally implementing a browser CSS/layout engine.

## Portable style IR

Define a deliberately bounded style vocabulary covering high-value portable concepts such as font family/size/weight, foreground/background, spacing, alignment, visibility, min/preferred/max dimensions, border/radius where meaningful, and simple state selectors.

Preserve Svelte component scoping identity and translate supported CSS rules into a style IR. Evaluate Lightning CSS as a parser/normalizer where useful, but do not let a browser CSS optimizer define native semantics.

Unsupported declarations/selectors emit compile-time diagnostics with a backend/portable explanation. No silent drops in strict mode.

## Backends

- GTK: map to GtkCssProvider/native style classes/properties where semantically sound.
- QtWidgets: QSS, palettes, fonts and direct widget properties as appropriate.
- egui: `Style`, `Visuals`, spacing/text styles and per-widget renderer parameters.

Provide explicit backend-native stylesheet/style-object escape hatches for applications that choose backend lock-in.

## Acceptance

The reference app has a portable style layer that produces recognizably equivalent hierarchy/spacing/typography across three backends, plus one backend-native styling example each. Style updates participate in HMR without rebuilding native code. Unsupported CSS fixtures produce deterministic diagnostics.
