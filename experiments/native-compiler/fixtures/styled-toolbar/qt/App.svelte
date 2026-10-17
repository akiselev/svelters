<!-- Qt-vocabulary copy of ../App.svelte (the qt compile target). -->
<!-- Native styling (ST1): setter properties (native names per toolkit) are matched at compile
     time and lowered to setProp; CSS properties go to the X6 translator (GTK CSS / QSS) with the
     classes sent through host.style. Precedence: explicit attribute > style= / style: > rules by
     (specificity, order) > native default. -->
<script>
  let active = $state(false);
</script>

<main-window window-title="Styled">
  <box-layout class="toolbar">
    <push-button class="primary">Save</push-button>
    <push-button class:primary={active} onclicked={() => (active = !active)}>Toggle</push-button>
    <push-button style="height-request: 30px; minimum-height: 30px; font-weight: bold">Plain</push-button>
  </box-layout>
</main-window>

<style>
  .toolbar {
    direction: left-to-right;
    spacing: 8px;
  }

  push-button {
    height-request: 24px;
    minimum-height: 24px;
  }

  .primary {
    height-request: 36px;
    minimum-height: 36px;
    color: #1c71d8;
  }
</style>
