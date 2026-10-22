<script>
  // AU2: crates/svelters-automation/tests/ipc.rs breaks the runaway loop behind "Spin" with
  // `app.interrupt` from the transport's IO thread, then checks the app still answers.
  let bumps = $state(0);

  function spin() {
    while (true) {}
  }
</script>

<window title="Hang" default-width={300} default-height={200}>
  <box class="column">
    <button onclicked={spin}>Spin</button>
    <button onclicked={() => bumps++}>Bump</button>
    <label>Bumps: {bumps}</label>
  </box>
</window>
