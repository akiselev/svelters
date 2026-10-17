<!-- kinds: QCalendarWidget QDateEdit QTimeEdit QDateTimeEdit -->
<script>
  // Calendar and date/time edits. Dates are {year, month, day} records, times {hour, minute,
  // second, ms}, date-times JS Dates. The date/time edits are two-way bound: the app writes new
  // values after mount, which must not come back as change events (echo labels stay 0).
  // Item views are covered by the portable virtual-list; QGraphicsView, QSplashScreen, QRhiWidget,
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

<box-layout direction="top-to-bottom" spacing="4">
  <calendar-widget bind:this={cal} minimum-date={{ year: 2024, month: 1, day: 1 }} maximum-date={{ year: 2024, month: 12, day: 31 }} selected-date={{ year: 2024, month: 3, day: 15 }} grid-visible={true}
            navigation-bar-visible={true} first-day-of-week="monday" horizontal-header-format="short-day-names"
            vertical-header-format="no-vertical-header" date-edit-enabled={true}
            onselection-changed={() => selected++} oncurrent-page-changed={() => pages++} />
  <label text="selection changes: {selected}, page changes: {pages}" />
  <date-edit minimum-date={{ year: 2024, month: 1, day: 1 }} maximum-date={{ year: 2024, month: 12, day: 31 }} bind:date display-format="yyyy-MM-dd" calendar-popup={true} ondate-changed={() => { if (armed) echoDate++; }} />
  <label text="echo date: {echoDate}" />
  <time-edit minimum-time={{ hour: 8, minute: 0, second: 0, ms: 0 }} maximum-time={{ hour: 18, minute: 0, second: 0, ms: 0 }} bind:time display-format="HH:mm" ontime-changed={() => { if (armed) echoTime++; }} />
  <label text="echo time: {echoTime}" />
  <date-time-edit bind:date-time={dateTime} display-format="yyyy-MM-dd HH:mm" ondate-time-changed={() => { if (armed) echoDateTime++; }} />
  <label text="echo date-time: {echoDateTime}" />
</box-layout>
