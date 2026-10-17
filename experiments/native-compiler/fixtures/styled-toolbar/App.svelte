<!-- Native styling (ST1): setter properties (native names per toolkit) are matched at compile
     time and lowered to setProp; CSS properties go to the X6 translator (GTK CSS / QSS) with the
     classes sent through host.style. Precedence: explicit attribute > style= / style: > rules by
     (specificity, order) > native default. -->
<script>
  let active = $state(false);
</script>

<window title="Styled">
  <box class="toolbar">
    <button class="primary">Save</button>
    <button class:primary={active} onclicked={() => (active = !active)}>Toggle</button>
    <button style="height-request: 30px; minimum-height: 30px; font-weight: bold">Plain</button>
  </box>
</window>

<style>
  .toolbar {
    orientation: horizontal;
    spacing: 8px;
  }

  button {
    height-request: 24px;
    minimum-height: 24px;
  }

  .primary {
    height-request: 36px;
    minimum-height: 36px;
    color: #1c71d8;
  }
</style>
