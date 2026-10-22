<script>
  let { label = 'OK' } = $props();
</script>

<box>
  <button text={label} on:clicked />
</box>
