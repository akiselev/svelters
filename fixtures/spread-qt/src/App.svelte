<script>
  // Qt twin of fixtures/spread (Qt vocabulary): a spread and <svelte:element this> on native
  // elements, resolved at run time against the spec tables.
  let clicks = $state(0);
  let step = $state(0);
  // what the spread holds at each step: props (a string coerced to the prop's type) and an event,
  // then other values and fewer keys (the missing ones go back to the explicit value, the default or
  // the native value read before the first write), then nothing
  const steps = [
    { text: 'Spread', 'tool-tip': 'from a spread', 'minimum-height': '30', onclicked: () => clicks++ },
    { text: 'Fewer keys', onclicked: () => (clicks += 10) },
    {},
  ];
  let extra = $derived(steps[step]);
  let tag = $derived(['label', 'push-button', null][step]);
</script>

<main-window window-title="Spread">
  <box-layout direction="top-to-bottom" spacing="4">
    <push-button text="Explicit" enabled={true} {...extra} flat={false} />
    <label text={`clicks ${clicks}`} />
    <svelte:element this={tag} text="dynamic" tool-tip={`step ${step}`} />
    <push-button onclicked={() => (step = Math.min(step + 1, 2))}>Next</push-button>
  </box-layout>
</main-window>
