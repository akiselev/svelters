<!-- kinds: QCalendarWidget QDateEdit QTimeEdit QDateTimeEdit -->
<script>
  // Calendar and date/time edits. Dates are {year, month, day} records, times {hour, minute, second, ms}, date-times JS Dates. Bindings on the date edits
  // are not annotated yet (gap: no generated no-echo test for non-scalar props), so values are
  // set declaratively and changes are counted by plain handlers.
  // Item views are covered by the portable virtuallist; QGraphicsView, QSplashScreen, QRhiWidget,
  // QRubberBand, QFocusFrame, QSizeGrip and QSplitterHandle are not exposed.
  let cal = $state(null);
  let selected = $state(0);
  let pages = $state(0);
  let dates = $state(0);
  let times = $state(0);
  let dts = $state(0);
  const day = { year: 2024, month: 3, day: 15 };
</script>

<box orientation="vertical" spacing="4">
  <calendar bind:this={cal} selectedDate={day} gridVisible={true}
            navigationBarVisible={true} firstDayOfWeek="monday" horizontalHeaderFormat="short-day-names"
            verticalHeaderFormat="no-vertical-header" dateEditEnabled={true}
            onselection-changed={() => selected++} onpage-changed={() => pages++} />
  <label label="selection changes: {selected}, page changes: {pages}" />
  <dateedit date={day} displayFormat="yyyy-MM-dd" calendarPopup={true} ondate-changed={() => dates++} />
  <label label="date changes: {dates}" />
  <timeedit time={{ hour: 9, minute: 30, second: 0, ms: 0 }} displayFormat="HH:mm" ontime-changed={() => times++} />
  <label label="time changes: {times}" />
  <datetimeedit dateTime={new Date(2024, 2, 15, 9, 30)} displayFormat="yyyy-MM-dd HH:mm"
                ondate-time-changed={() => dts++} />
  <label label="date-time changes: {dts}" />
</box>
