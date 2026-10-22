<script>
  // TR3 (SV14): the same, with a `tick(t)` transition: JS runs per frame and writes the opacity.
  // crates/svelters-egui/tests/tick_profile.rs loads it from source and measures the outro.
  
  let show = $state(true);
  let rows = Array.from({ length: 40 }, (_, i) => i);
  function tickFade(node) {
    return { duration: 400, tick: (t) => { node.style.opacity = String(t); } };
  }
</script>

<window title="Profile" default-width={400} default-height={900}>
  <box class="column">
    <button onclicked={() => (show = !show)}>Toggle</button>
    {#if show}
      {#each rows as r (r)}
        <label out:tickFade|global>Row {r}</label>
      {/each}
    {/if}
  </box>
</window>
