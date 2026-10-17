<script>
  let { label, done = $bindable(false), onremove } = $props();
</script>

<box-layout direction="left-to-right" spacing="4">
  <label>{done ? '✓' : '·'} {label}</label>
  <push-button onclicked={() => (done = !done)}>Toggle</push-button>
  <push-button onclicked={onremove}>Remove</push-button>
</box-layout>
