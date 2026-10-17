<window class="w">
  <box class="toolbar" id="main">
    <button class="primary">a</button>
    <label class="x">x</label>
  </box>
  <box-layout class="toolbar" id="main">
    <push-button class="primary">a</push-button>
  </box-layout>
</window>

<style>
  .toolbar { color: #333; spacing: 8px; orientation: horizontal; direction: left-to-right; }
  box#main, box-layout#main { spacing: 12; }
  button { width-request: 80; minimum-width: 80; }
  push-button { minimum-width: 80; }
  .toolbar .primary { hexpand: true; }
  .primary:hover { hexpand: true; color: blue; }
  label { spacing: 4; }
  .x { stretch: 1; halign: start; contents-margins: 4 8; }
  @media (prefers-color-scheme: dark) { .toolbar { spacing: 2; } }
  .toolbar { & .x { margin-start: 4; } }
  .primary { halign: middle; margin-top: 4px; }
</style>
