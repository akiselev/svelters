<!-- Qt-vocabulary copy of ../App.svelte (the qt compile target). -->
<!-- bind:text on a native entry: metadata-driven binding plan (no wrapper component). -->
<script>
  let name = $state('');
  let edits = $state(0);
</script>

<main-window window-title="Binding">
  <box-layout direction="top-to-bottom" spacing="6">
    <line-edit placeholder-text="Your name" bind:text={name} ontext-changed={() => edits++} />
    <label>{name === '' ? 'Nobody' : `Hello ${name}`} ({edits} edits)</label>
    <push-button onclicked={() => (name = '')}>Clear</push-button>
    <push-button onclicked={() => (name = name.toUpperCase())}>Shout</push-button>
  </box-layout>
</main-window>
