<script>
  // Native Qt models feeding @svelters/lists (NON-GUI-TYPES section 9; N2 acceptance, N3 model mode).
  // Each ListView shows its model natively: the QListView reads the QAbstractItemModel, JS holds no copy
  // of the rows. A QStandardItemModel's rows are its QStandardItem wrappers (one per item); any other
  // model's rows are QModelIndex values read through the model. A row re-renders when the model reports
  // dataChanged for it, so `it.text()` and `names.data(index)` stay current without a mirror.
  import { ListView } from '@svelters/lists';
  import { QStringListModel } from 'svelters:qt/core';
  import { QStandardItemModel, QStandardItem } from 'svelters:qt/gui';

  const names = new QStringListModel();
  names.setStringList(['alpha', 'beta']);
  const items = new QStandardItemModel();
  items.appendRow(new QStandardItem('one'));
  items.appendRow(new QStandardItem('two'));

  // The row count only (Qt's rowCount() has no change signal of its own).
  const count = () => names.rowCount() + items.rowCount();
  let rows = $state(count());
  for (const m of [names, items]) {
    for (const s of ['rowsInserted', 'rowsRemoved', 'modelReset']) m.on(s, () => (rows = count()));
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
      <label>rows={rows}</label>
    </box-layout>
    <ListView items={names}>
      {#snippet row(index)}
        <label>{`names ${names.data(index)}`}</label>
      {/snippet}
    </ListView>
    <ListView items={items}>
      {#snippet row(it)}
        <label>{`items ${it.text()}`}</label>
      {/snippet}
    </ListView>
  </box-layout>
</main-window>
