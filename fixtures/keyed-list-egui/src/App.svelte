<script>
  // Egui twin of keyed-list: text-edit for entry, a plain box for the scrolled window (scroll-area
  // is G2). The native `changed` event carries the text, as GTK's does.
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
  <box gap={6}>
    <box flex-direction="row" gap={6}>
      <text-edit hint-text="New item" text={draft} onchanged={(arg) => (draft = arg?.detail ?? arg)} />
      <button onclicked={add}>Add</button>
      <button onclicked={reverse}>Reverse</button>
    </box>
    <box>
      <box gap={2}>
        {#each rows as row (row.id)}
          <box flex-direction="row" gap={6}>
            <label>{row.text}</label>
            <button onclicked={() => remove(row.id)}>Remove</button>
          </box>
        {/each}
      </box>
    </box>
  </box>
</window>
