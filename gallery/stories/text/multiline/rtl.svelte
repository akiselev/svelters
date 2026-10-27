<script>
  let { disabled = false } = $props();
</script>

<box padding={8}>
  <text-edit anchor="subject" multiline height={96} accessible-name="Notes" text={'السطر الأول\nالسطر الثاني مع English كلمات'} enabled={!disabled} />
</box>
