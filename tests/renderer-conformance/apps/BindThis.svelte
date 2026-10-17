<script>
  // bind:this is rejected with a custom renderer; {@attach} gives the node instead.
  let el = $state();
  let show = $state(true);
  export function get() { return el; }
  export function toggle() { show = !show; }
  const ref = (n) => { el = n; return () => { el = undefined; }; };
</script>
{#if show}<entry {@attach ref} />{/if}
