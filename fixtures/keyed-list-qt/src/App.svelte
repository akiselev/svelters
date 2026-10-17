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

<main-window window-title="Keyed list">
  <box-layout direction="top-to-bottom" spacing="6">
    <box-layout direction="left-to-right" spacing="6">
      <line-edit placeholder-text="New item" text={draft} ontext-changed={(arg) => (draft = arg?.detail ?? arg)} />
      <push-button onclicked={add}>Add</push-button>
      <push-button onclicked={reverse}>Reverse</push-button>
    </box-layout>
    <scroll-area>
      <box-layout direction="top-to-bottom" spacing="2">
        {#each rows as row (row.id)}
          <box-layout direction="left-to-right" spacing="6">
            <label>{row.text}</label>
            <push-button onclicked={() => remove(row.id)}>Remove</push-button>
          </box-layout>
        {/each}
      </box-layout>
    </scroll-area>
  </box-layout>
</main-window>
