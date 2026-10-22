<script>
  // EV1 (crates/svelters-egui/tests/events.rs, loaded from source only like FocusDialog.svelte):
  // Svelte listeners on egui's DOM-style events, in capture, target and bubble order. The outer
  // box logs its phases (C1 capture, B3 bubble), the inner box `c` (capture) and `b` (bubble),
  // the button `T` (at the target) and `!` (its own `clicked`, after the pass).
  let log = $state('');
  let stop = $state(false);
  let prevent = $state(false);
  const add = (s) => (log += s);
</script>

<window title="Events" default-width={360} default-height={260}>
  <box onclickcapture={(e) => add(`C${e.eventPhase}`)} onclick={(e) => add(`B${e.eventPhase}`)}>
    <box
      padding={12}
      on:click|capture={() => add('c')}
      onclick={(e) => {
        add('b');
        if (stop) e.stopPropagation();
      }}
    >
      <button
        onclick={(e) => add(e.target === e.currentTarget && e.eventPhase === 2 ? 'T' : '?')}
        onpointerdown={(e) => {
          if (prevent) e.preventDefault();
        }}
        onclicked={() => add('!')}>Child</button
      >
    </box>
    <button onclicked={() => (stop = true)}>Stop</button>
    <button onclicked={() => (prevent = true)}>Prevent</button>
    <button onclicked={() => (log = '')}>Clear</button>
    <label>Log: {log}</label>
  </box>
</window>
