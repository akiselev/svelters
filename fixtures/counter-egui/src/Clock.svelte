<script>
  // AU1b: crates/svelters-automation/tests/e2e.rs. "Start" arms a 30 s timer; on the virtual
  // clock it fires after 30 s of frames, not 30 s of wall time. The rolls come from `Math.random`,
  // which the session's seed makes replayable.
  let fired = $state('no');
  let rolls = $state(Array.from({ length: 3 }, () => Math.floor(Math.random() * 1000)).join(' '));
  let t0 = performance.now();

  function start() {
    t0 = performance.now();
    setTimeout(() => (fired = `after ${Math.round(performance.now() - t0)} ms`), 30_000);
  }
</script>

<window title="Clock" default-width={300} default-height={200}>
  <box class="column">
    <button onclicked={start}>Start</button>
    <label>Fired: {fired}</label>
    <label>Rolls: {rolls}</label>
  </box>
</window>
