<!-- bind:text on a native entry: metadata-driven binding plan (no wrapper component). -->
<script>
  let name = $state('');
  let edits = $state(0);
</script>

<window title="Binding">
  <box orientation="vertical" spacing="6">
    <entry placeholder="Your name" bind:text={name} onchanged={() => edits++} />
    <label>{name === '' ? 'Nobody' : `Hello ${name}`} ({edits} edits)</label>
    <button onclick={() => (name = '')}>Clear</button>
    <button onclick={() => (name = name.toUpperCase())}>Shout</button>
  </box>
</window>
