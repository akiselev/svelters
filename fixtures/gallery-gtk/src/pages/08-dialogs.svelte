<!-- kinds: Window Popover AboutDialog EmojiChooser -->
<script>
  let closed = $state(0);
  let picked = $state('none');
  // Toplevels are declared where they belong, in `if` blocks inside the window. The renderer makes
  // them transient for the enclosing window and shows them (dialogs: modal + present).
  let aboutOpen = $state(true);
  let aboutClosed = $state(0);
  let toolOpen = $state(true);
  let toolClosed = $state(0);
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
  <box orientation="horizontal" spacing="6">
    <button label="about" onclick={() => (aboutOpen = true)} />
    <label label="about closed: {aboutClosed}" />
    <button label="tool window" onclick={() => (toolOpen = true)} />
    <label label="tool window closed: {toolClosed}" />
  </box>
  {#if aboutOpen}
    <aboutdialog
      programName="svelters gallery"
      version="0.1"
      comments="A toplevel declared in an if block"
      licenseType="mit-x11"
      onclose={() => {
        aboutOpen = false;
        aboutClosed++;
      }}
    />
  {/if}
  {#if toolOpen}
    <window
      title="Tool window"
      defaultWidth={280}
      defaultHeight={120}
      onclose={() => {
        toolOpen = false;
        toolClosed++;
      }}
    >
      <label label="a second window, transient for the main one" />
    </window>
  {/if}
</box>
