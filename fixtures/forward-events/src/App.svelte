<script>
  // FW1: the grandparent. Toolbar's button forwards `clicked` (bare `on:clicked`), Panel forwards
  // it again (bare `on:clicked` on <Toolbar>), and the handler passed here gets the event. The
  // handler is swapped after the first click: the forwarded prop is read when the event fires.
  // The second Panel gets no handler: its click does nothing.
  import Panel from './Panel.svelte';
  let count = $state(0);
  const inc = () => {
    count += 1;
    handler = dec;
  };
  const dec = () => {
    count -= 10;
  };
  let handler = $state(inc);
</script>

<window title="Forward">
  <box orientation="vertical">
    <label label="Count: {count}" />
    <Panel title="With handler" onclicked={handler} />
    <Panel title="Without handler" />
  </box>
</window>
