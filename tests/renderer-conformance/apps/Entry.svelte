<script>
  // Native entry wrapper: two-way via $bindable + native 'changed' signal + 'text' prop.
  let { value = $bindable(''), ...rest } = $props();
</script>
<entry text={value} onchanged={(t) => (value = t)} {...rest} />
