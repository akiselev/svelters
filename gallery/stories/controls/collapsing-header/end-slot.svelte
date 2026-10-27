<script>
  let { disabled = false } = $props();
</script>

<box padding={8}>
  <collapsing-header anchor="subject" text="Layers" open enabled={!disabled}>
    <label>Top copper</label>
    <label>Bottom copper</label>
    <button data-slot="end" text="Add" small />
  </collapsing-header>
</box>
