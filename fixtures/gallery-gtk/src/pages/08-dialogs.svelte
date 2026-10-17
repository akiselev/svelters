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
    <menu-button label="popover">
      <popover autohide={true} has-arrow={true} position="bottom" cascade-popdown={true} {...{ onclosed: () => closed++ }}>
        <label label="inside the popover" />
      </popover>
    </menu-button>
    <label label="popover closed: {closed}" />
  </box>
  <box orientation="horizontal" spacing="6">
    <menu-button label="emoji">
      <emoji-chooser {...{ "onemoji-picked": (e) => (picked = String(e)) }} />
    </menu-button>
    <label label="picked: {picked}" />
  </box>
  <box orientation="horizontal" spacing="6">
    <button label="about" onclicked={() => (aboutOpen = true)} />
    <label label="about closed: {aboutClosed}" />
    <button label="tool window" onclicked={() => (toolOpen = true)} />
    <label label="tool window closed: {toolClosed}" />
  </box>
  {#if aboutOpen}
    <about-dialog
      program-name="svelters gallery"
      version="0.1"
      comments="A toplevel declared in an if block"
      license-type="mit-x11"
      onclose-request={() => {
        aboutOpen = false;
        aboutClosed++;
      }}
    />
  {/if}
  {#if toolOpen}
    <window
      title="Tool window"
      default-width={280}
      default-height={120}
      onclose-request={() => {
        toolOpen = false;
        toolClosed++;
      }}
    >
      <label label="a second window, transient for the main one" />
    </window>
  {/if}
</box>
