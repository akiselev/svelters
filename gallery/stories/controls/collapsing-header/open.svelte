<script>
  let { open = true, disabled = false } = $props();
</script>

<box padding={8}>
  <collapsing-header anchor="subject" text="Advanced" {open} enabled={!disabled}>
    <checkbox text="Use the experimental router" />
    <checkbox text="Keep backups" checked />
  </collapsing-header>
</box>
