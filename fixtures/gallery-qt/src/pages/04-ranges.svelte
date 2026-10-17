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

<box-layout direction="top-to-bottom" spacing="4">
  <slider orientation="horizontal" minimum={0} maximum={100} tick-position="ticks-below" tick-interval={10} bind:value={level} onvalue-changed={() => echoSlider++} />
  <label text="echo slider: {echoSlider}" />
  <dial notches-visible={true} wrapping={false} maximum-height={70} bind:value={angle} onvalue-changed={() => echoDial++} />
  <label text="echo dial: {echoDial}" />
  <scroll-bar orientation="horizontal" bind:value={scroll} onvalue-changed={() => echoScroll++} />
  <label text="echo scroll: {echoScroll}" />
  <spin-box minimum={0} maximum={20} prefix="n=" suffix=" items" bind:value={count} onvalue-changed={() => echoSpin++} />
  <label text="echo spin: {echoSpin}" />
  <double-spin-box minimum={0} maximum={10} decimals={2} single-step={0.5} bind:value={ratio} onvalue-changed={() => echoDouble++} />
  <label text="echo double: {echoDouble}" />
  <progress-bar minimum={0} maximum={100} value={level} format="%p%" text-visible={true} />
  <lcd-number digit-count={4} int-value={count} />
</box-layout>
