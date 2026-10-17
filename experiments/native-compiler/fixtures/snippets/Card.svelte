<script>
  let { title, body, value, children, footer } = $props();
</script>

<box orientation="vertical" spacing="2">
  <label>{title} #{value}</label>
  {@render body(value)}
  {@render children?.()}
  {@render footer?.()}
</box>
