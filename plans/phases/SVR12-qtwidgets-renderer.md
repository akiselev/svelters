# SVR12 - QtWidgets renderer and selective C++ thunks

References: `plans/architecture/qt-shiboken.md`, `plans/architecture/native-binding-linker.md`.

## Goal

Deliver a real QtWidgets backend with full ordinary-method access through selective generated C++ thunks. Qt Quick is explicitly out of scope.

## Native architecture

Use `QApplication::exec()` as the authoritative loop. Remove the old 10 ms `processEvents` polling design and do not revive the libuv event-dispatcher experiment.

Maintain a typed QObject/native-value handle registry. QObject deletion invalidates handles. Parent ownership changes update the registry according to API IR semantics.

Use QMetaObject for properties/signals/invokable methods where appropriate, but ordinary C++ API coverage comes from generated thunks selected by the binding manifest.

## Thin generator

Generate a C++ translation unit/table containing only live methods/constructors plus required converters/adapters. Build with the platform's normal Qt toolchain/Ninja/CMake integration and section GC/LTO as appropriate.

Separately generate/cache a fat development bridge per Qt/API/toolchain fingerprint so adding a normal Qt method in Svelte does not compile C++ during iteration.

## Initial widget set

QApplication, QWidget/QDialog, QMainWindow, common layouts, QLabel, QPushButton, QLineEdit/QTextEdit, check/radio, splitter, scroll area, tabs, menus/actions/status bar, and enough item-view support for pre-SVR16 smoke tests.

## Acceptance

- port the old Qt Svelte counter behavior to v2;
- reference editor-shell fixture runs under QtWidgets;
- call at least one ordinary non-Q_INVOKABLE Qt method from Svelte through a generated binding;
- adding that call in dev does not rebuild C++ when the fat bridge already covers it;
- release manifest/thin bridge contains it and omits an unreachable control method;
- QObject deletion/parent-transfer/lifetime tests pass under ASan/UBSan where supported.
