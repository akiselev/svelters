<script>
  // SP1: a spread and <svelte:element this> on native elements, resolved at run time against the
  // spec tables (docs/notes/native-compiler/runtime-contract.md "Spread attributes").
  let clicks = $state(0);
  let step = $state(0);
  // what the spread holds at each step: props (a string coerced to the prop's type) and an event,
  // then other values and fewer keys (the missing ones go back to the explicit value or the
  // default), then nothing
  const steps = [
    { label: 'Spread', 'tooltip-text': 'from a spread', 'width-request': '120', onclicked: () => clicks++ },
    { label: 'Fewer keys', onclicked: () => (clicks += 10) },
    {},
  ];
  let extra = $derived(steps[step]);
  let tag = $derived(['label', 'button', null][step]);
</script>

<window title="Spread">
  <box orientation="vertical" spacing="4">
    <button label="Explicit" sensitive={true} {...extra} halign="center" />
    <label label={`clicks ${clicks}`} />
    <svelte:element this={tag} label="dynamic" tooltip-text={`step ${step}`} />
    <button onclicked={() => (step = Math.min(step + 1, 2))}>Next</button>
  </box>
</window>
