<script>
  // Native Qt models feeding @svelters/lists (NON-GUI-TYPES section 9, N2 acceptance). The ListView reads
  // a QStringListModel and a QStandardItemModel through their generated classes and follows their native
  // signals; rows of the item model are keyed by the QStandardItem wrapper itself (one wrapper per item).
  import { ListView } from '@svelters/lists';
  import { QStringListModel } from 'svelters:qt/core';
  import { QStandardItemModel, QStandardItem } from 'svelters:qt/gui';

  const names = new QStringListModel();
  names.setStringList(['alpha', 'beta']);
  const items = new QStandardItemModel();
  items.appendRow(new QStandardItem('one'));
  items.appendRow(new QStandardItem('two'));

  const read = () => [
    ...names.stringList().map((text, i) => ({ key: 'n' + i, text: `names ${text}` })),
    ...Array.from({ length: items.rowCount() }, (_, i) => {
      const it = items.item(i);
      return { key: it, text: `items ${it.text()}` };
    }),
  ];
  let rows = $state(read());
  const refresh = () => (rows = read());
  for (const m of [names, items]) {
    for (const s of ['rowsInserted', 'rowsRemoved', 'dataChanged', 'modelReset']) m.on(s, refresh);
  }

  let n = 0;
  function addName() {
    const at = names.rowCount();
    names.insertRows(at, 1);
    names.setData(names.index(at, 0), 'gamma' + n++);
  }
  function addItem() {
    items.appendRow(new QStandardItem('three'));
  }
  function rename() {
    items.item(0).setText('uno');
  }
  function removeFirst() {
    items.removeRow(0);
  }
</script>

<main-window window-title="Models">
  <box-layout direction="top-to-bottom" spacing="6">
    <box-layout direction="left-to-right" spacing="6">
      <push-button onclicked={addName}>addName</push-button>
      <push-button onclicked={addItem}>addItem</push-button>
      <push-button onclicked={rename}>rename</push-button>
      <push-button onclicked={removeFirst}>removeFirst</push-button>
      <label>rows={rows.length}</label>
    </box-layout>
    <ListView items={rows} key={(r) => r.key}>
      {#snippet row(r)}
        <label>{r.text}</label>
      {/snippet}
    </ListView>
  </box-layout>
</main-window>
