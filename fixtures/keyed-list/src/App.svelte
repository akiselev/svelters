<script>
  let draft = $state('');
  let nextId = 1;
  let rows = $state([]);

  function add() {
    if (draft === '') return;
    rows.push({ id: nextId++, text: draft });
    draft = '';
  }

  function remove(id) {
    rows = rows.filter((r) => r.id !== id);
  }

  function reverse() {
    rows = rows.toReversed();
  }
</script>

<window title="Keyed list">
  <box orientation="vertical" spacing="6">
    <box orientation="horizontal" spacing="6">
      <entry placeholder-text="New item" text={draft} onchanged={(arg) => (draft = arg?.detail ?? arg)} />
      <button onclicked={add}>Add</button>
      <button onclicked={reverse}>Reverse</button>
    </box>
    <scrolled-window>
      <box orientation="vertical" spacing="2">
        {#each rows as row (row.id)}
          <box orientation="horizontal" spacing="6">
            <label>{row.text}</label>
            <button onclicked={() => remove(row.id)}>Remove</button>
          </box>
        {/each}
      </box>
    </scrolled-window>
  </box>
</window>
