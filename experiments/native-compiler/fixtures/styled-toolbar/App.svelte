<!-- Native styling: declarations lower to native props / native appearance per backend.
     Precedence: explicit attribute > rules by (specificity, order) > native default. -->
<script>
  let active = $state(false);
</script>

<window title="Styled">
  <box class="toolbar">
    <button class="primary">Save</button>
    <button class:primary={active} onclick={() => (active = !active)}>Toggle</button>
    <button>Plain</button>
  </box>
</window>

<style>
  .toolbar {
    orientation: horizontal;
    spacing: 8dp;
  }

  button {
    min-height: 24dp;
  }

  .primary {
    min-height: 36dp;
    emphasis: accent;
  }
</style>
