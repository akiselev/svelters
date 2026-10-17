<script>
  let open = $state(true);
  let result = $state('waiting');
  let about = $state();

  function finished(code) {
    result = `finished ${code}`;
    open = false;
  }
  function showAbout() {
    about.call('present');
  }
</script>

<window title="Main">
  <box orientation="vertical" spacing="8">
    <label>{result}</label>
    <button onclick={() => (open = true)}>Open</button>
    <button onclick={showAbout}>About</button>
    {#if open}
      <messagebox text="Save changes?" onfinished={finished} />
    {/if}
  </box>
</window>

<aboutdialog show="none" bind:this={about} programName="Dialogs" />
