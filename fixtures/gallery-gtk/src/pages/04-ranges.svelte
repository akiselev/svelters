<!-- kinds: ProgressBar LevelBar Scale SpinButton Scrollbar Range -->
<script>
  let fraction = $state(0.25);
  let level = $state(0.5);
  let levelEchoes = $state(0);
  let spin = $state(5);
  let spinEchoes = $state(0);
  let scaleValue = $state(10);
  let scaleEchoes = $state(0);
  let scaleChanges = $state(0);
  let offsets = $state(0);
  let bar = $state();
  let scale = $state();
  let spinner = $state();
  $effect(() => {
    // range and increments are plain props now (the adjustment behind them is forwarded by the backend)
    bar?.call('pulse');
    // programmatic writes after mount: must not come back as change events
    level = 0.75;
    spin = 7;
    scaleValue = 30;
    fraction = 0.5;
  });
</script>

<box orientation="vertical" spacing="6">
  <progress-bar bind:this={bar} fraction={fraction} text="Loading" show-text={true} pulse-step={0.1} inverted={false} ellipsize="end" hexpand={true} />
  <box orientation="horizontal" spacing="6">
    <level-bar bind:value={() => level, (v) => { level = v; levelEchoes++; }} min-value={0} max-value={1} mode="discrete" inverted={false} hexpand={true} {...{ 'onoffset-changed': () => offsets++ }} />
    <label label="echo level: {levelEchoes}" />
  </box>
  <box orientation="horizontal" spacing="6">
    <scale bind:this={scale} lower={0} upper={100} step-increment={1} page-increment={10} bind:value={() => scaleValue, (v) => { scaleValue = v; scaleEchoes++; }} orientation="horizontal" digits={1} draw-value={true} has-origin={true} value-pos="right" show-fill-level={true} fill-level={60} restrict-to-fill-level={false} inverted={false} hexpand={true} onvalue-changed={() => scaleChanges++} />
    <label label="scale changes: {scaleChanges}" />
    <label label="echo scale: {scaleEchoes}" />
  </box>
  <box orientation="horizontal" spacing="6">
    <spin-button bind:this={spinner} lower={0} upper={100} step-increment={1} page-increment={10} bind:value={() => spin, (v) => { spin = v; spinEchoes++; }} digits={0} numeric={true} wrap={true} snap-to-ticks={false} climb-rate={1} update-policy="always" />
    <label label="echo spin: {spinEchoes}" />
  </box>
  <scrollbar orientation="horizontal" />
</box>
