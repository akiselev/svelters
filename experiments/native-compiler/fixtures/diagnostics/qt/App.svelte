<!-- Qt-vocabulary copy of ../App.svelte (the qt compile target). -->
<!-- Deliberately invalid: every line exercises one native validation. -->
<script>
  let count = $state(0);
  let flag = $state(true);
</script>

<main-window window-title="Diagnostics">
  <box-layout direction="diagonal" spacing="4">
    hello
    <label bind:text={count}>x</label>
    <push-button onklick={() => count++}>Go</push-button>
    <line-edit colour="red" />
    <frobnicator />
  </box-layout>
</main-window>

<style>
  .x { color: red; }
  box-layout { spacing: 4dp; }
  push-button:hover { height-request: 10px; minimum-height: 10px; }
  .x .y { spacing: 2px; }
</style>
