<script>
  let count = $state(0);
  let snap = $state.snapshot(count);
  let eager = $derived($state.eager(count));
  $effect(() => {
    console.log(count, $effect.tracking(), $effect.pending());
  });
  $effect.pre(() => {
    count;
  });
  const stop = $effect.root(() => {
    $effect(() => {});
    return () => {};
  });
  function later() {
    let local = $state(1);
    local++;
    return local;
  }
  for (let i = 0; i < 2; i++) {
    count += i;
  }
</script>

<label label={`${count} ${eager} ${snap}`} />
