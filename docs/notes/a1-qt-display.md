# A1 qt-display notes

- Date/time edit bindings (`bind:date` etc.) dropped: `host_suppressed` bindings need `suppress_signals`, whose generated test (`gen_cpp.py` `sample()`) only supports scalar props, not date/time. New gap: no-echo test for non-scalar props.
- `minimumDate/maximumDate` (QCalendarWidget, QDateEdit, QDateTimeEdit) and min/max Time/DateTime dropped: the ingested property metadata resolves the getter to `clearMinimumDate()` (returns void), so the generated C++ does not compile; a prop-level `getter:` override is ignored. New gap: ingest picks the RESET function as getter.
- QCalendarWidget `selectedDate` binding not possible: `selectionChanged()` has no argument for `change.value: arg0`.
- Item views, QGraphicsView, QSplashScreen, QRhiWidget, QRubberBand, QFocusFrame, QSizeGrip, QSplitterHandle, QUndoView, QHeaderView marked unexposed.
