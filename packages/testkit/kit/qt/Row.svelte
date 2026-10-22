<script>
  let { gap = 6, children } = $props();
</script>

<box-layout direction="left-to-right" spacing={gap}>
  {@render children?.()}
</box-layout>
