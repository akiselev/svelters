<!-- kinds: DatePicker TimePicker CalendarDatePicker CalendarView ColorPicker -->
<stack-panel orientation="vertical" spacing="6">
  <date-picker header="Date picker" />
  <time-picker header="Time picker" clock-identifier="24HourClock" />
  <calendar-date-picker header="Calendar date picker" placeholder-text="Pick a date" />
  <calendar-view selection-mode="single" height={320} />
  <color-picker color="#3080ff" is-alpha-enabled={true} />
</stack-panel>
