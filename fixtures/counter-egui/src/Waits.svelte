<script>
  // AU7: crates/svelters-automation/tests/waits.rs's actionability checks. "Mover" moves on every
  // 16 ms tick of the virtual clock (never stable); "Cover" lies on top of "Under".
  let x = $state(0);
  setInterval(() => (x = (x + 7) % 140), 16);
</script>

<window title="Waits" default-width={400} default-height={300}>
  <box class="column">
    <button margin-left={x}>Mover</button>
    <box class="stack">
      <button width={200} height={40}>Under</button>
      <button class="cover">Cover</button>
    </box>
    <label>Still</label>
  </box>
</window>

<style>
  .column {
    gap: 8px;
  }
  .stack {
    position: relative;
    width: 200px;
    height: 40px;
  }
  .cover {
    position: absolute;
    left: 0;
    top: 0;
    width: 200px;
    height: 40px;
  }
</style>
