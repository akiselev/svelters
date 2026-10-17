<!-- Deliberately invalid: every line exercises one native validation. -->
<script>
  let count = $state(0);
  let flag = $state(true);
</script>

<window title="Diagnostics">
  <box orientation="diagonal" spacing="4">
    hello
    <label bind:label={count}>x</label>
    <button onklick={() => count++}>Go</button>
    <entry colour="red" />
    <frobnicator />
  </box>
</window>

<style>
  .x { color: red; }
  box { spacing: 4px; }
  button:hover { min-height: 10dp; }
</style>
