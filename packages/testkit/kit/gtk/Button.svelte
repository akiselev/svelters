<script>
  let { text = '', onclick } = $props();
</script>

<button label={text} onclicked={() => onclick?.()} />
