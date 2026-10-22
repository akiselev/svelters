<script>
  let { placeholder = '', value = $bindable('') } = $props();
</script>

<entry placeholder-text={placeholder} bind:text={value} />
