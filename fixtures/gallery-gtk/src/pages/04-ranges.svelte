<!-- kinds: ProgressBar LevelBar Scale SpinButton Scrollbar Range -->
<script>
  let fraction = $state(0.25);
  let level = $state(0.5);
  let levelEchoes = $state(0);
  let spin = $state(5);
  let spinEchoes = $state(0);
  let scaleChanges = $state(0);
  let offsets = $state(0);
  let bar = $state();
  let scale = $state();
  let spinner = $state();
  $effect(() => {
    // the default adjustment is 0..0: the app sets the range first
    scale?.call('set_range', 0, 100);
    spinner?.call('set_range', 0, 100);
    spinner?.call('set_increments', 1, 10);
    bar?.call('pulse');
    // programmatic writes after mount: must not come back as change events
    level = 0.75;
    spin = 7;
    fraction = 0.5;
  });
</script>

<box orientation="vertical" spacing="6">
  <progressbar bind:this={bar} fraction={fraction} text="Loading" show-text={true} pulse-step={0.1} inverted={false} ellipsize="end" hexpand={true} />
  <box orientation="horizontal" spacing="6">
    <levelbar bind:value={() => level, (v) => { level = v; levelEchoes++; }} min-value={0} max-value={1} mode="discrete" inverted={false} hexpand={true} {...{ 'onoffset-changed': () => offsets++ }} />
    <label label="echo level: {levelEchoes}" />
  </box>
  <box orientation="horizontal" spacing="6">
    <scale bind:this={scale} orientation="horizontal" digits={1} draw-value={true} has-origin={true} value-pos="right" show-fill-level={true} fill-level={60} restrict-to-fill-level={false} inverted={false} hexpand={true} onvalue-changed={() => scaleChanges++} />
    <label label="scale changes: {scaleChanges}" />
  </box>
  <box orientation="horizontal" spacing="6">
    <spinbutton bind:this={spinner} bind:value={() => spin, (v) => { spin = v; spinEchoes++; }} digits={0} numeric={true} wrap={true} snap-to-ticks={false} climb-rate={1} update-policy="always" />
    <label label="echo spin: {spinEchoes}" />
  </box>
  <scrollbar orientation="horizontal" />
</box>
