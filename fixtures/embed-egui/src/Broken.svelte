<script>
  // EM1: throws during its first render.
  let { reason = 'boom' } = $props();
  throw new Error(`Broken: ${reason}`);
</script>

<label>never shown</label>
