<script>
  let { text = '', checked = $bindable(false) } = $props();
</script>

<checkbox {text} bind:checked />
