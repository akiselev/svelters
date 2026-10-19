<script>
  let dirty = $state(true);
  let status = $state('idle');
  let keys = $state(0);
</script>

<window
  title="Sync"
  onclose-request={(e) => {
    if (dirty) {
      e.preventDefault();
      status = 'vetoed';
    } else {
      status = 'closing';
    }
  }}
>
  <box orientation="vertical" spacing="4">
    <label>{status} / keys {keys}</label>
    <entry>
      <!-- GTK delivers key presses through an event controller attached to the widget (X8) -->
      <event-controller-key
        onkey-pressed={(e, keyval) => {
          keys++;
          if (keyval === 97) e.preventDefault();
          if (keyval === 99) e.result = false;
        }}
      />
    </entry>
    <button onclicked={() => (dirty = !dirty)}>Toggle dirty</button>
  </box>
</window>
