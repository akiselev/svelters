<!-- kinds: Window Popover AboutDialog EmojiChooser -->
<script>
  let closed = $state(0);
  let picked = $state('none');
</script>

<box orientation="vertical" spacing="8">
  <box orientation="horizontal" spacing="6">
    <menubutton label="popover">
      <popover autohide={true} hasArrow={true} position="bottom" cascadePopdown={true} {...{ onclosed: () => closed++ }}>
        <label label="inside the popover" />
      </popover>
    </menubutton>
    <label label="popover closed: {closed}" />
  </box>
  <box orientation="horizontal" spacing="6">
    <menubutton label="emoji">
      <emojichooser {...{ "onemoji-picked": (e) => (picked = String(e)) }} />
    </menubutton>
    <label label="picked: {picked}" />
  </box>
  <label label="AboutDialog is a toplevel: verified by generated tests, not mounted here" />
</box>
