<script>
  // bind:text on a native entry. `changes` counts the native `changed` signals that reach JS:
  // only natively made edits may count, never the app's own writes (echo suppression).
  let name = $state('Ann');
  let changes = $state(0);
</script>

<window title="bind:text">
  <box orientation="vertical" spacing="6">
    <entry bind:text={name} onchanged={() => changes++} />
    <button onclicked={() => (name = 'Bob')}>Set Bob</button>
    <button onclicked={() => (name = name.toUpperCase())}>Upper</button>
    <label>Hello {name}</label>
    <label>changes: {changes}</label>
  </box>
</window>
