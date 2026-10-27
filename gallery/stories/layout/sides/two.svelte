<script>
  let { align = 'center' } = $props();
</script>

<sides anchor="subject" align-items={align} padding={8} gap={8}>
  <label>Board: main</label>
  <button text="Run checks" />
</sides>
