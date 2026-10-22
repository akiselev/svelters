<!-- kinds: Hyperlink RadioButton Switch RangeSlider DragValue -->
<script>
  // WG2b inputs: every bound value is echoed by a label, so tests/gallery.rs can check the binding.
  let choice = $state('a');
  let first = $state(true);
  let second = $state(false);
  let wifi = $state(false);
  let low = $state(10);
  let high = $state(90);
  let width = $state(3);
  let clicks = $state(0);
</script>

<box gap={6}>
  <hyperlink url="https://svelte.dev" onclicked={() => clicks++}>Svelte</hyperlink>
  <label>link clicks: {clicks}</label>

  <box flex-direction="row" gap={8}>
    <radio-button value="a" bind:group={choice}>Alpha</radio-button>
    <radio-button value="b" bind:group={choice}>Beta</radio-button>
    <radio-button value="c" bind:group={choice}>Gamma</radio-button>
  </box>
  <label>group: {choice}</label>

  <box flex-direction="row" gap={8}>
    <radio-button bind:checked={first}>First</radio-button>
    <radio-button bind:checked={second}>Second</radio-button>
  </box>
  <label>checked: {first} {second}</label>

  <switch bind:active={wifi} align-self="start">Wi-Fi</switch>
  <label>active: {wifi}</label>

  <range-slider text="Range" min={0} max={100} step={1} bind:low bind:high />
  <label>range: {low} to {high}</label>

  <drag-value min={0} max={10} suffix=" mm" accessible-name="Width" align-self="start" bind:value={width} />
  <label>width: {width}</label>
</box>
