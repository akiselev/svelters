<script>
  let count = $state(0);
</script>

<window title="Counter">
  <box orientation="vertical" spacing="8">
    <label>Count: {count}</label>
    <button onclick={() => count++}>Increment</button>
  </box>
</window>
