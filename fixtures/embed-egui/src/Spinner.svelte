<script>
  // EM1: an event handler that never returns (interrupted by the embed's handler budget).
  let spins = $state(0);
  function spin() {
    spins++;
    for (;;) {}
  }
</script>

<box>
  <label>Spinner ready</label>
  <button onclicked={spin}>Spin</button>
</box>
