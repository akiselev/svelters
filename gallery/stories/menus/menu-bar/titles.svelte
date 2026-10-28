<script>
  let { disabled = false } = $props();
</script>

<box flex-grow={1}>
  <menu-bar anchor="subject" enabled={!disabled}>
    <menu label="&File">
      <action label="New board" shortcut="Ctrl+N" />
      <action label="Open…" shortcut="Ctrl+O" />
    </menu>
    <menu label="&Edit">
      <action label="Undo" shortcut="Ctrl+Z" />
    </menu>
    <menu label="&View" enabled={false}>
      <action label="Zoom in" />
    </menu>
    <menu label="&Place">
      <action label="Footprint" />
    </menu>
    <menu label="&Route">
      <action label="Autoroute selected nets" />
    </menu>
    <menu label="&Help">
      <action label="About" />
    </menu>
  </menu-bar>
</box>
