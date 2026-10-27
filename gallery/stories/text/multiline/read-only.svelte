<script>
  let { disabled = false } = $props();
</script>

<box padding={8}>
  <text-edit anchor="subject" multiline read-only height={96} accessible-name="License" text={'MIT License\n\nPermission is hereby granted, free of charge, to any person obtaining a copy.'} enabled={!disabled} />
</box>
