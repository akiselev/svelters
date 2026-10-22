<script>
  // TR3 (SV14): 40 labels with transition:fade (css descriptors: keyframes sent once, the scene plays them).
  // crates/svelters-egui/tests/tick_profile.rs loads it from source and measures the outro.
  import { fade } from 'svelte/transition';
  let show = $state(true);
  let rows = Array.from({ length: 40 }, (_, i) => i);
  
</script>

<window title="Profile" default-width={400} default-height={900}>
  <box class="column">
    <button onclicked={() => (show = !show)}>Toggle</button>
    {#if show}
      {#each rows as r (r)}
        <label out:fade|global={{ duration: 400 }}>Row {r}</label>
      {/each}
    {/if}
  </box>
</window>
