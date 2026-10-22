<script>
  let { text = '', checked = $bindable(false) } = $props();
</script>

<check-button label={text} bind:active={checked} />
