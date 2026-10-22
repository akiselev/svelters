<script>
  // WG4a: a <toast-host> and its JS API: `show` through bind:this, `closed`/`clicked` back.
  let host = $state();
  let n = 0;
  let last = $state('none');
  let clicked = $state(-1);
</script>

<box gap={4}>
  <toast-host bind:this={host} duration={0} onclosed={(id, reason) => (last = `${id} ${reason}`)} onclicked={(id) => (clicked = id)} />
  <button onclicked={() => host.call('show', `Hello ${++n}`, 'success')}>Notify</button>
  <button onclicked={() => host.call('clear')}>Clear all</button>
  <label>Closed: {last}</label>
  <label>Clicked: {clicked}</label>
</box>
