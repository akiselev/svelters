<script>
  let { label, count = 0, onpress, children } = $props();
</script>
<button text={label} count={count} onclick={() => onpress?.(count)}>{@render children?.()}</button>
