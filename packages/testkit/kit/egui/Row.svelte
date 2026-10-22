<script>
  let { gap = 6, children } = $props();
</script>

<box flex-direction="row" align-items="center" {gap}>
  {@render children?.()}
</box>
