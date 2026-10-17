<!-- kinds: QCalendarWidget QDateEdit QTimeEdit QDateTimeEdit -->
<script>
  // Calendar and date/time edits. Dates are {year, month, day} records, times {hour, minute,
  // second, ms}, date-times JS Dates. The date/time edits are two-way bound: the app writes new
  // values after mount, which must not come back as change events (echo labels stay 0).
  // Item views are covered by the portable virtuallist; QGraphicsView, QSplashScreen, QRhiWidget,
  // QRubberBand, QFocusFrame, QSizeGrip and QSplitterHandle are not exposed.
  let cal = $state(null);
  let selected = $state(0);
  let pages = $state(0);
  let date = $state({ year: 2024, month: 3, day: 15 });
  let time = $state({ hour: 9, minute: 30, second: 0, ms: 0 });
  let dateTime = $state(new Date(2024, 2, 15, 9, 30));
  let echoDate = $state(0);
  let echoTime = $state(0);
  let echoDateTime = $state(0);
  let armed = false;
  $effect(() => {
    setTimeout(() => {
      armed = true;
      date = { year: 2024, month: 6, day: 1 };
      time = { hour: 10, minute: 15, second: 0, ms: 0 };
      dateTime = new Date(2024, 5, 1, 10, 15);
    }, 0);
  });
</script>

<box orientation="vertical" spacing="4">
  <calendar bind:this={cal} minimumDate={{ year: 2024, month: 1, day: 1 }} maximumDate={{ year: 2024, month: 12, day: 31 }} selectedDate={{ year: 2024, month: 3, day: 15 }} gridVisible={true}
            navigationBarVisible={true} firstDayOfWeek="monday" horizontalHeaderFormat="short-day-names"
            verticalHeaderFormat="no-vertical-header" dateEditEnabled={true}
            onselection-changed={() => selected++} onpage-changed={() => pages++} />
  <label label="selection changes: {selected}, page changes: {pages}" />
  <dateedit minimumDate={{ year: 2024, month: 1, day: 1 }} maximumDate={{ year: 2024, month: 12, day: 31 }} bind:date displayFormat="yyyy-MM-dd" calendarPopup={true} ondate-changed={() => { if (armed) echoDate++; }} />
  <label label="echo date: {echoDate}" />
  <timeedit minimumTime={{ hour: 8, minute: 0, second: 0, ms: 0 }} maximumTime={{ hour: 18, minute: 0, second: 0, ms: 0 }} bind:time displayFormat="HH:mm" ontime-changed={() => { if (armed) echoTime++; }} />
  <label label="echo time: {echoTime}" />
  <datetimeedit bind:dateTime displayFormat="yyyy-MM-dd HH:mm" ondate-time-changed={() => { if (armed) echoDateTime++; }} />
  <label label="echo dateTime: {echoDateTime}" />
</box>
