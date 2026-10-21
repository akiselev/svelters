<script>
  // R2, real-host only (examples/counter/tests/integration.rs `bind_observe_qt`): `bind:size` on a
  // box-layout is read-only and exact on Qt (the Resize event) -> `$n.bind_readonly`. Until the Qt
  // backend observes events (R3) the host answers that it cannot observe it, and the binding keeps the
  // values read at creation and after the first layout pass. No op log: the op-log hosts' Qt schema has
  // no read-only props.
  let size = $state(null);
</script>

<main-window window-title="bind readonly">
  <box-layout direction="top-to-bottom" spacing="6" bind:size>
    <label>size {size ? `${size.width}x${size.height}` : 'none'}</label>
  </box-layout>
</main-window>
