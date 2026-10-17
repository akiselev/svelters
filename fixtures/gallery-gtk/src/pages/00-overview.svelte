<!-- kinds: Box Label Button Entry ScrolledWindow -->
<script>
  // Overview: the P elements. Each family page (NN-family.svelte) shows its kinds with their
  // annotated props, signals, slots and bindings. Bindings show `echo <name>: <count>` labels
  // that count change events; the screenshot test requires them to stay 0 after the app's own
  // writes (echo suppression, see fixtures/bind-value).
  let name = $state('world');
  let echoes = $state(0);
  let clicks = $state(0);
  $effect(() => {
    // a programmatic write after mount: must not come back as a change event
    name = 'gallery';
  });
</script>

<box orientation="vertical" spacing="6">
  <label label="Hello {name}" />
  <entry bind:text={name} onchanged={() => echoes++} />
  <label label="echo text: {echoes}" />
  <button label="Clicked {clicks} times" onclick={() => clicks++} />
</box>
