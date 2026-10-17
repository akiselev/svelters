# A1 qt-base notes

- Bindings cannot live in `inherit:` (schema), so `bind:checked` / `bind:value` are not annotated here; checkable/slider elements add them in their batches.
- `QAbstractButton::checked` cannot pass the generated prop test on a non-checkable element (isChecked() stays false), so `button` has `exclude: [checked]`. No element currently tests `checked` (checkbox batch will). Likewise `QAbstractSlider`/`QAbstractSpinBox`/`QFrame` facts are declared but have no element yet, so they are unverified until qt-sliders/qt-spinboxes/frames elements exist (the spinbox, combobox etc. elements existing today derive from QAbstractSpinBox/QFrame where applicable and do exercise them).
- `pressed`/`released` triggers: `setDown()` emits nothing; a QMouseEvent sent with QCoreApplication::sendEvent does (wrap in braces, the trigger is pasted into a shared function scope).
- QPushButton.yaml duplicates (label, shortcut, checkable, click, clicked) were removed; the inherited versions are equivalent.
- Reverted the QPushButton duplicates removal: the docs IR (tools/docs test_qt_frontend) does not see inherited props on elements, so the button element keeps its own label/click/clicked/checkable/shortcut (gap: docs IR ignores inherit blocks).
