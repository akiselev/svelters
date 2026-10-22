<script>
  let { gap = 6, children } = $props();
</script>

<box orientation="horizontal" spacing={gap}>
  {@render children?.()}
</box>
