# SVR18 - broad native API escape hatch, callbacks, async, and converters

References: `plans/architecture/native-binding-linker.md`, `plans/architecture/native-api-ir.md`.

## Goal

Move from renderer-enough toolkit access to a broadly useful native API surface while preserving selective linking and explicit lifetime semantics.

## Scope

Expand QtCore/QtGui/QtWidgets and GTK/Gio/Gdk coverage needed for dialogs, clipboard, drag/drop, actions/menus, timers, tray integration, painting, shortcuts, models, custom data values and other desktop facilities.

Complete callback/trampoline semantics in the IR:

- sync callback that must return before the native call continues;
- event/listener callback queued into JS;
- one-shot async completion;
- cancellable async operation;
- callback scopes/lifetime/invalidation.

Where a native API genuinely requires synchronous JS reentry, document and test the reentrancy contract rather than routing it through the normal queued event model.

Implement converter generation for strings, enums/flags, object handles, common value types, option/null, slices/arrays, maps where supported, opaque values and backend-specific data. Prefer explicit host-owned value handles before clever zero-copy representations.

## Dynamic use

Finish explicit manual roots/patterns and development tracing. Add reports for dynamic-call cost and unsupported signatures. No fallback that indiscriminately links a whole toolkit module in release.

## Acceptance

A desktop-integration fixture uses file dialog, clipboard, menu/action, shortcut, timer and one async native operation through both Qt and GTK where equivalent facilities exist. Binding manifests remain selective. Callback-after-destroy, cancellation and ownership tests pass.
