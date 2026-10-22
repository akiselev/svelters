<script>
  // TR1 on egui: transitions through the visual channels (opacity, transform, clip, inert) and the
  // frame clock. crates/svelters-egui/tests/motion.rs drives it under kittest.
  import { fade, fly, slide } from 'svelte/transition';
  let show = $state(true);
  let ended = $state(0);
</script>

<window title="Motion" default-width={360} default-height={260}>
  <box class="column">
    <button onclicked={() => (show = !show)}>Toggle</button>
    {#if show}
      <label transition:fade={{ duration: 120 }} onoutroend={() => ended++}>Faded</label>
      <button transition:fly={{ duration: 120, y: 20 }}>Flown</button>
      <label transition:slide={{ duration: 120 }}>Slid</label>
    {/if}
    <label>Outros ended: {ended}</label>
  </box>
</window>

<style>
  .column {
    gap: 8px;
  }
</style>
