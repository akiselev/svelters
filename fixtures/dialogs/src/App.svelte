<script>
  // Toplevel kinds are declared where they belong: a second window in an `if` block is transient
  // for the window around it; an about dialog created with show="none" is opened by the app.
  let open = $state(true);
  let about = $state();
</script>

<window title="Main">
  <box orientation="vertical" spacing="8">
    <label>{open ? 'settings open' : 'settings closed'}</label>
    <button onclick={() => (open = !open)}>Toggle</button>
    <button onclick={() => about.call('present')}>About</button>
    {#if open}
      <window title="Settings">
        <label>Settings body</label>
      </window>
    {/if}
  </box>
</window>

<aboutdialog show="none" bind:this={about} programName="Dialogs" />
