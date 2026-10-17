# A1 qt-selection notes

- delivered_once runs the trigger several times and needs exactly one emission each: QComboBox emits currentIndexChanged on the first addItem, so triggers add one item and only change the index from the second call on. currentTextChanged needs unique item texts (same text = no signal).
- Index props on empty widgets: prop `setup: $w->addItems({"a","b","c"})` then sample 1.
- Gallery checker: every label starting "echo " must end in ": 0"; put other counters in separate labels.
- Gallery adds items with bind:this + call('addItem', ...) after mount.
- QTreeWidget, QTableWidget (G7/G6) and QButtonGroup (not a widget) marked unexposed. QListView/QTreeView-style: use virtuallist (X5).
