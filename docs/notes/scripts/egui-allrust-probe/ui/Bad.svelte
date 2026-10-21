<script>
  let count = $state(0);
</script>

<window title="Bad">
  <box orientation="vertical">
    <buton onclick={() => count++}>Increment</buton>
    <label bogus-prop="1">x</label>
  </box>
</window>
