<script>
  let { label, done = $bindable(false), onremove } = $props();
</script>

<box orientation="horizontal" spacing="4">
  <label>{done ? '✓' : '·'} {label}</label>
  <button onclick={() => (done = !done)}>Toggle</button>
  <button onclick={onremove}>Remove</button>
</box>
