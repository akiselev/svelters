<script>
  let { label, done = $bindable(false), onremove } = $props();
</script>

<box orientation="horizontal" spacing="4">
  <label>{done ? '✓' : '·'} {label}</label>
  <button onclicked={() => (done = !done)}>Toggle</button>
  <button onclicked={onremove}>Remove</button>
</box>
