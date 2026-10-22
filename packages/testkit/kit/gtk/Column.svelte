<script>
  let { gap = 6, children } = $props();
</script>

<box orientation="vertical" spacing={gap}>
  {@render children?.()}
</box>
