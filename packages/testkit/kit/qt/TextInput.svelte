<script>
  let { placeholder = '', value = $bindable('') } = $props();
</script>

<line-edit placeholder-text={placeholder} bind:text={value} />
