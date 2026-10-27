<script>
  let { caption = 'Save', disabled = false } = $props();
</script>
<button anchor="subject" text={caption} {disabled} />
