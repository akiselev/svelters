<script>
  // WG4b: drag and drop on any node. `drag-type` + `drag-data` make a source, `drop-types` a target
  // (`*` any type, `cad/*` a prefix). The target is outlined while a drag it accepts is over it.
  let parts = $state([
    { id: 1, name: 'R1 10k' },
    { id: 2, name: 'C3 100n' },
    { id: 3, name: 'U2 STM32' },
  ]);
  let placed = $state([]);
  let hovering = $state(false);
  let status = $state('drag a part onto the board');

  function drop(type, data) {
    const part = JSON.parse(data);
    placed.push(part);
    parts = parts.filter((p) => p.id !== part.id);
    status = `placed ${part.name} (${type})`;
  }
</script>

<box class="row">
  <box class="col">
    <label>Parts</label>
    {#each parts as part (part.id)}
      <button drag-type="cad/part" drag-data={JSON.stringify(part)} ondrag-end={(dropped) => !dropped && (status = 'drag cancelled')}>
        {part.name}
      </button>
    {/each}
  </box>
  <box
    class="board"
    drop-types="cad/*"
    ondrag-enter={() => (hovering = true)}
    ondrag-leave={() => (hovering = false)}
    ondrop={drop}
  >
    <label>{hovering ? 'Board: drop here' : 'Board'}</label>
    {#each placed as part (part.id)}
      <label>{part.name}</label>
    {/each}
  </box>
  <box class="col" drop-types="text/plain">
    <label>Notes (accepts text/plain only)</label>
  </box>
</box>
<label>{status}</label>

<style>
  .row {
    flex-direction: row;
    gap: 12px;
  }
  .col {
    gap: 4px;
  }
  .board {
    gap: 4px;
    padding: 8px;
    min-width: 200px;
    min-height: 160px;
  }
</style>
