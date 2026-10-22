<script>
  let { text = '', onclick } = $props();
</script>

<button {text} onclicked={() => onclick?.()} />
