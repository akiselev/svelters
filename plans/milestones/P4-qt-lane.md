# P4 - Qt lane

Owns `crates/svelters-qt-build` and `examples/counter-qt`.

## Work
- Hand-written C++17 backend implementing `svl_backend` for Qt 6.11 Widgets. A handle table of `QPointer<QObject>`; a `destroyed` connection calls `host.on_destroyed`.
- Kinds: `window` (QMainWindow, `setCentralWidget` as default slot), `box` (QWidget + QVBoxLayout/QHBoxLayout by `orientation`, ordered insert via `insertWidget`), `label` (QLabel `text`), `button` (QPushButton `text`, `clicked`), `entry` (QLineEdit `text`, `textChanged`).
- Write a dispatch switch by `svl_id` in the same shape the M3 generator will emit, so that M3 can replace it mechanically. Use QMetaProperty-by-name where it works, and explicit C++ calls where it doesn't (layouts, `setCentralWidget`).
- Loop: `QApplication::exec`. `wake` is `QMetaObject::invokeMethod(obj, ..., Qt::QueuedConnection)`, which is thread-safe. The checkpoint runs from it.
- `svelters-qt-build`: a `Config` builder used from a leaf crate's `build.rs`. Finds Qt via pkg-config (`Qt6Widgets`), runs moc only if needed (prefer no `Q_OBJECT` in P), compiles with `cc`, and emits link directives and narrow `rerun-if-changed` lines.
- `examples/counter-qt`: the leaf crate exposing `backend() -> &'static svl_backend`.

## Acceptance
- Under `xvfb-run`, a Rust test drives the Qt backend with the same scripted op sequence as P3 and verifies the widget tree and `clicked` delivery.
- Touching a Rust source in a dependent crate doesn't rerun the leaf crate's build script. Check with `cargo build -vv` output.
- The counter window appears from a hand-written op sequence.
