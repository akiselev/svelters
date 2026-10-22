<script>
  // EM1: with `cycle`, an animation-frame loop that never ends (an injected cycle for
  // run_until_settled); without it, one frame and then settled.
  let { cycle = false } = $props();
  let ticks = $state(0);
  $effect(() => {
    let id;
    const tick = () => {
      ticks++;
      if (cycle) id = requestAnimationFrame(tick);
    };
    id = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(id);
  });
</script>

<label>Ticks {ticks > 0 ? 'started' : 'waiting'}</label>
