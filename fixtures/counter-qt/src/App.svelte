<script>
  // Qt twin of fixtures/counter (Qt vocabulary).
  let count = $state(0);
</script>

<main-window window-title="Counter">
  <box-layout direction="top-to-bottom" spacing="8">
    <label>Count: {count}</label>
    <push-button onclicked={() => count++}>Increment</push-button>
  </box-layout>
</main-window>
