<script>
  let { disabled = false } = $props();
</script>

<box padding={12}>
  <progress-bar anchor="subject" fraction={0.4} text="جارٍ التحميل" enabled={!disabled} />
</box>
