<!-- Deliberately invalid: every line exercises one native validation. -->
<script>
  let count = $state(0);
  let flag = $state(true);
</script>

<window title="Diagnostics">
  <box orientation="diagonal" spacing="4">
    hello
    <inscription bind:text={count} />
    <button onklick={() => count++}>Go</button>
    <entry colour="red" />
    <frobnicator />
  </box>
</window>

<style>
  .x { color: red; }
  box { spacing: 4dp; }
  button:hover { height-request: 10px; minimum-height: 10px; }
  .x .y { spacing: 2px; }
</style>
