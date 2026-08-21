# SVR09 - GTK4 API extraction and dynamic invocation

References: `plans/architecture/native-api-ir.md`, `plans/architecture/native-binding-linker.md`.

## Goal

Replace the old hand-maintained TS GIR parser/generated Rust surface with a modern GTK/GLib API database and a development-time dynamic invocation layer.

## Extraction

Use modern GIRepository/typelib/GIR information to populate the canonical IR for GLib/GObject/Gio/Gdk/Gtk modules required by the renderer. Preserve source typelib/GIR versions and hashes.

Map:

- GType hierarchy/interfaces;
- properties and notify semantics;
- signals/callback argument metadata;
- functions, methods, constructors and virtual methods;
- argument direction/nullability/ownership transfer/callback scope;
- enums/flags/constants;
- since/deprecated annotations where available.

Generate `@svelters/gtk` TypeScript declarations from the IR.

## Dynamic development path

Implement:

1. generic GObject get/set for representable properties;
2. generic signal connection/disconnection with safe callback handles;
3. GIRepository callable invocation for supported methods/functions/constructors;
4. explicit classification for skipped/non-introspectable/varargs/caller-allocated/unsupported cases.

Do not generate thousands of Rust wrappers to claim coverage.

## Coverage report

Produce counts and lists for `Reflected`, `Introspected`, `GeneratedThunkRequired`, `CustomAdapter`, and `Unsupported`, including reason codes.

## Acceptance

A standalone GTK probe creates objects, gets/sets properties, connects a signal, and invokes at least one non-property method through GI metadata using canonical binding IDs. Generated typings compile representative GTK-native Svelte source. API extraction is deterministic for a pinned typelib set.
