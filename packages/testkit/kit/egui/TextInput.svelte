<script>
  let { placeholder = '', value = $bindable('') } = $props();
</script>

<text-edit hint-text={placeholder} bind:text={value} />
