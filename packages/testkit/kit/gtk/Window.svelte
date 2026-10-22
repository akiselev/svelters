<script>
  let { title = '', width = 360, height = 260, children } = $props();
</script>

<window {title} default-width={width} default-height={height}>
  {@render children?.()}
</window>
