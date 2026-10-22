<script>
  let { text = '', onclick } = $props();
</script>

<push-button {text} onclicked={() => onclick?.()} />
