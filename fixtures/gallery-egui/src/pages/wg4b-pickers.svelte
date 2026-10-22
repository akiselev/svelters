<script>
  // WG4b: `date-picker` (egui_extras) and `color-picker` (egui core), both with `bind:value`.
  let date = $state('2026-10-22');
  let color = $state('#3366cc');
  let tint = $state('#ff000080');
</script>

<box class="col">
  <box class="row">
    <label>Due date</label>
    <date-picker bind:value={date} accessible-name="Pick the due date" />
    <label>Due: {date}</label>
  </box>
  <box class="row">
    <label>Layer colour</label>
    <color-picker bind:value={color} accessible-name="Layer colour" />
    <label>{color}</label>
  </box>
  <box class="row">
    <label>Highlight (with alpha)</label>
    <color-picker alpha="blend" bind:value={tint} accessible-name="Highlight" />
    <label>{tint}</label>
  </box>
  <date-picker value="" format="%d %b %Y" calendar-week={false} tooltip="Empty value: shows today" />
</box>

<style>
  .col {
    gap: 8px;
  }
  .row {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }
</style>
