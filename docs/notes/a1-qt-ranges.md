# a1-qt-ranges
- gen_bindings.py (qt) resolves `property`/`change.signal` only from the element's own props/signals, not `inherit:`. Sliders/dial/scrollbar therefore redeclare `value` (suppress_signals) and `valueChanged` on the element; the inherited versions are shadowed there.
- Signal arg names must be the qdoc parameter names (QSpinBox `i`, QDoubleSpinBox `d`).
- All inherited QAbstractSlider/QAbstractSpinBox facts passed unchanged.
