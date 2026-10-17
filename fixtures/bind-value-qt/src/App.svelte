<script>
  // Qt twin of fixtures/bind-value. bind:text on a native line-edit. `changes` counts the native
  // `text-changed` signals that reach JS: only natively made edits may count, never the app's own
  // writes (echo suppression).
  let name = $state('Ann');
  let changes = $state(0);
</script>

<main-window window-title="bind:text">
  <box-layout direction="top-to-bottom" spacing="6">
    <line-edit bind:text={name} ontext-changed={() => changes++} />
    <push-button onclicked={() => (name = 'Bob')}>Set Bob</push-button>
    <push-button onclicked={() => (name = name.toUpperCase())}>Upper</push-button>
    <label>Hello {name}</label>
    <label>changes: {changes}</label>
  </box-layout>
</main-window>
