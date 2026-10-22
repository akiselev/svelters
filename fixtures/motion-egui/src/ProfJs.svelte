<script>
  // TR3 (SV14): the same, with the native animator switched off: TR1's JS player sends one style op per channel per frame.
  // crates/svelters-egui/tests/tick_profile.rs loads it from source and measures the outro.
  import { fade } from 'svelte/transition';
  globalThis.__svelters_no_native_animation = true;
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
