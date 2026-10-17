<!-- kinds: QSlider QDial QScrollBar QSpinBox QDoubleSpinBox QProgressBar QLCDNumber -->
<script>
  // Range widgets: sliders, dial, scrollbar, spin boxes (two-way value bindings), progress, LCD.
  let level = $state(10);
  let angle = $state(20);
  let scroll = $state(30);
  let count = $state(4);
  let ratio = $state(1.5);
  let echoSlider = $state(0);
  let echoDial = $state(0);
  let echoScroll = $state(0);
  let echoSpin = $state(0);
  let echoDouble = $state(0);
  $effect(() => {
    // programmatic writes after mount: must not come back as change events
    level = 40;
    angle = 60;
    scroll = 50;
    count = 7;
    ratio = 2.5;
  });
</script>

<box orientation="vertical" spacing="4">
  <slider orientation="horizontal" minimum={0} maximum={100} tickPosition="ticks-below" tickInterval={10} bind:value={level} onvalueChanged={() => echoSlider++} />
  <label label="echo slider: {echoSlider}" />
  <dial notchesVisible={true} wrapping={false} maximumHeight={70} bind:value={angle} onvalueChanged={() => echoDial++} />
  <label label="echo dial: {echoDial}" />
  <scrollbar orientation="horizontal" bind:value={scroll} onvalueChanged={() => echoScroll++} />
  <label label="echo scroll: {echoScroll}" />
  <spinbox minimum={0} maximum={20} prefix="n=" suffix=" items" bind:value={count} onvalueChanged={() => echoSpin++} />
  <label label="echo spin: {echoSpin}" />
  <doublespinbox minimum={0} maximum={10} decimals={2} singleStep={0.5} bind:value={ratio} onvalueChanged={() => echoDouble++} />
  <label label="echo double: {echoDouble}" />
  <progressbar minimum={0} maximum={100} value={level} format="%p%" textVisible={true} />
  <lcdnumber digitCount={4} intValue={count} />
</box>
