<script>
  let { gap = 6, children } = $props();
</script>

<box-layout direction="top-to-bottom" spacing={gap}>
  {@render children?.()}
</box-layout>
