# A1 qt-buttons notes

- QCheckBox/QRadioButton hit-test mouse events against the indicator rect: the inherited pressed/released triggers
  (pos 2,2 on a 0x0 widget) never fire. Both elements redeclare pressed/released with resize(200,40) and pos (10,20).
- Inherited toggled trigger (`setChecked(i%2==0)`) breaks the binding test (it runs the trigger with $i=1, which unchecks
  an unchecked box = no emission) and on a radio an exclusive uncheck is ignored. checkbox/radiobutton redeclare toggled
  with a flipping trigger (radio also `setAutoExclusive(false)`).
- gen_bindings.py (qt_bindings) reads only the element's own props/signals, not inherited ones: `checked` is redeclared
  on checkbox/radiobutton. Brief says inherited props count; they do not (gap for the lead).
- checkState is not a Q_PROPERTY: annotated as `type: enum` getter/setter with a `values` map (kebab-case nicks in JS).
- checkStateChanged(Qt::CheckState): not tried, enum signal args; stateChanged(int) annotated instead.
- toolbutton setMenu(object<QMenu>): resolution test only.
