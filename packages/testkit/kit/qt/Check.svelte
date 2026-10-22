<script>
  let { text = '', checked = $bindable(false) } = $props();
</script>

<check-box {text} bind:checked />
