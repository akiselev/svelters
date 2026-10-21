<script>
  // Native Qt item models feeding @svelters/lists (N3), the twin of tests/gtk-types-models. Driven by
  // examples/counter-qt/tests/lists_models.rs: it clicks the buttons by label and watches the native view.
  import { ListView } from '@svelters/lists';
  import { QSortFilterProxyModel, QStringListModel } from 'svelters:qt/core';

  // A model mutated from JS: each mutation is one native row signal (and a dataChanged for setData).
  const store = new QStringListModel();
  let next = 0;
  const fill = () => store.setStringList(Array.from({ length: 10 }, () => `S${next++}`));
  const insert = () => {
    store.insertRows(2, 1);
    store.setData(store.index(2, 0), 'ins');
  };
  const remove = () => store.removeRows(0, 1);
  const rename = () => store.setData(store.index(0, 0), 'renamed');
  const sortDesc = () => store.sort(0, 'descending-order');

  // A filter/sort chain: a proxy over another model.
  const base = new QStringListModel();
  base.setStringList(['c', 'a', 'e', 'b', 'd', 'f']);
  const chain = new QSortFilterProxyModel();
  chain.setSourceModel(base);
  chain.setFilterRegularExpression('^[aceg]$');
  chain.sort(0, 'descending-order');
  const chainAdd = () => {
    const at = base.rowCount();
    base.insertRows(at, 1);
    base.setData(base.index(at, 0), 'g');
  };
  const refilter = () => chain.setFilterRegularExpression('^[bdf]$');

  let strings = $state.raw(null);
  const makeStrings = (n) => {
    const m = new QStringListModel();
    m.setStringList(Array.from({ length: n }, (_, i) => `Item ${i}`));
    strings = m;
    mode = 'strings';
  };
  let array = $state.raw([]);
  const makeArray = (n) => {
    array = Array.from({ length: n }, (_, i) => ({ id: i, text: `Item ${i}` }));
    mode = 'array';
  };

  let mode = $state('store');
  const model = $derived(mode === 'store' ? store : mode === 'chain' ? chain : mode === 'strings' ? strings : null);
  const items = $derived(model ?? array);

  let list = $state(null);
  let selected = $state(-1);
  let activated = $state('-');
  // Rows of a QStringListModel or a proxy are QModelIndex values; `model` reads them (current per dataChanged).
  const text = (item) => (item && typeof item === 'object' && 'text' in item ? item.text : model ? `${model.data(item)}` : '');
  const count = () => (model ? model.rowCount() : array.length);

  globalThis.__fixtureKeep = [store, base, chain];
</script>

<main-window window-title="Native models">
  <box-layout direction="top-to-bottom" spacing="4">
    <box-layout direction="left-to-right" spacing="4">
      <push-button onclicked={() => (mode = 'store')}>store</push-button>
      <push-button onclicked={fill}>fill</push-button>
      <push-button onclicked={insert}>insert</push-button>
      <push-button onclicked={remove}>remove</push-button>
      <push-button onclicked={rename}>rename</push-button>
      <push-button onclicked={sortDesc}>sort</push-button>
      <push-button onclicked={() => (mode = 'chain')}>chain</push-button>
      <push-button onclicked={chainAdd}>chainAdd</push-button>
      <push-button onclicked={refilter}>refilter</push-button>
    </box-layout>
    <box-layout direction="left-to-right" spacing="4">
      <push-button onclicked={() => makeStrings(10000)}>str10k</push-button>
      <push-button onclicked={() => makeStrings(100000)}>str100k</push-button>
      <push-button onclicked={() => makeArray(10000)}>arr10k</push-button>
      <push-button onclicked={() => makeArray(100000)}>arr100k</push-button>
      <push-button onclicked={() => list?.scrollTo(count() - 1)}>scrollEnd</push-button>
      <push-button onclicked={() => list?.scrollTo(Math.floor(count() / 2))}>scrollMid</push-button>
      <push-button onclicked={() => list?.scrollTo(0)}>scrollStart</push-button>
    </box-layout>
    <label>{`mode=${mode} selected=${selected} activated=${activated}`}</label>
    <ListView
      bind:this={list}
      {items}
      key={(it) => it.id}
      bind:selected
      vexpand={true}
      min-height={300}
      onactivate={(item, pos) => (activated = `${pos}:${text(item)}`)}
    >
      {#snippet row(item, index)}
        <box-layout direction="left-to-right" spacing="6">
          <label>{index}</label>
          <label>{text(item)}</label>
        </box-layout>
      {/snippet}
    </ListView>
  </box-layout>
</main-window>
