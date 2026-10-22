<script>
  let { title = '', width = 360, height = 260, children } = $props();
</script>

<main-window window-title={title} minimum-width={width} minimum-height={height}>
  {@render children?.()}
</main-window>
