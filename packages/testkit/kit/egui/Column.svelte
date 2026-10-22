<script>
  let { gap = 6, children } = $props();
</script>

<box {gap}>
  {@render children?.()}
</box>
