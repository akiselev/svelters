<script>
  // The egui gallery: one page per widget family of the built-in set (EGUI.md §5). Pages are added
  // by the WIDGETS lanes; `tests/gallery_egui.rs` in svelters-egui drives them under kittest.
  import Pickers from './pages/wg4b-pickers.svelte';
  import DragDrop from './pages/wg4b-dnd.svelte';
  import Dialogs from './pages/wg4b-dialogs.svelte';

  const pages = ['Pickers', 'Drag and drop', 'Dialogs'];
  let page = $state('Pickers');
</script>

<window title="egui gallery" default-width={640} default-height={480}>
  <box class="page">
    <box class="tabs">
      {#each pages as p}
        <button selected={page === p} onclicked={() => (page = p)}>{p}</button>
      {/each}
    </box>
    {#if page === 'Pickers'}
      <Pickers />
    {:else if page === 'Drag and drop'}
      <DragDrop />
    {:else}
      <Dialogs />
    {/if}
  </box>
</window>

<style>
  .page {
    gap: 12px;
    padding: 12px;
  }
  .tabs {
    flex-direction: row;
    gap: 4px;
  }
</style>
