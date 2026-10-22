<script>
  // TR2 on egui: a keyed list reordered with animate:flip (FLIP from the scene's bounds).
  // crates/svelters-egui/tests/animator.rs loads it from source.
  import { flip } from 'svelte/animate';
  let items = $state(['Alpha', 'Beta', 'Gamma']);
</script>

<window title="Flip" default-width={300} default-height={240}>
  <box class="column">
    <button onclicked={() => (items = [...items].reverse())}>Reverse</button>
    {#each items as item (item)}
      <label animate:flip={{ duration: 200 }}>{item}</label>
    {/each}
  </box>
</window>

<style>
  .column {
    gap: 8px;
  }
</style>
