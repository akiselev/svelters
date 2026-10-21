<script>
  // Native GTK list models feeding @svelters/lists (N3). Driven by crates/svelters-gtk/tests/lists_models.rs:
  // it clicks the buttons by label, types the directory path into the entry and watches the native view.
  import { ListView } from '@svelters/lists';
  import {
    Adjustment,
    CustomFilter,
    CustomSorter,
    DirectoryList,
    FilterListModel,
    SingleSelection,
    SortListModel,
    StringList,
    StringObject,
  } from 'svelters:gtk/gtk';
  import { File, FileInfo, ListStore } from 'svelters:gtk/gio';

  const so = (s) => StringObject.new(s);
  const str = (o) => StringObject.from(o).string;

  // A store mutated from JS: every mutation is one native items-changed.
  const store = new ListStore({ itemType: StringObject });
  let next = 0;
  const fill = () => store.splice(0, store.getNItems(), Array.from({ length: 10 }, () => so(`S${next++}`)));
  const insert = () => store.insert(2, so('ins'));
  const remove = () => store.remove(0);
  const splice = () => store.splice(1, 2, [so('x'), so('y'), so('z')]);
  // g_list_store_sort with a JS comparator (N3 unblocked GCompareDataFunc over list items).
  const sortDesc = () => store.sort((a, b) => (str(a) < str(b) ? 1 : str(a) > str(b) ? -1 : 0));

  // A filter/sort chain over another store, both with JS functions.
  const base = new ListStore({ itemType: StringObject });
  base.splice(0, 0, ['c', 'a', 'e', 'b', 'd', 'f'].map(so));
  let parity = 1;
  const filter = CustomFilter.new((o) => str(o).charCodeAt(0) % 2 === parity); // a, c, e
  const sorter = CustomSorter.new((a, b) => (str(a) < str(b) ? 1 : str(a) > str(b) ? -1 : 0));
  const chain = new FilterListModel({ model: new SortListModel({ model: base, sorter }), filter });
  const chainAdd = () => base.append(so('g'));
  const refilter = () => {
    parity = 1 - parity;
    filter.changed('different');
  };

  // Items with reactive getters: a row shows `adj.value`, which the test changes natively.
  const adjs = new ListStore({ itemType: Adjustment });
  adjs.splice(0, 0, [1, 2, 3].map((v) => new Adjustment({ lower: 0, upper: 100, value: v })));

  // A selection model of the app's own, shown as is.
  const sel = new SingleSelection({ model: store, autoselect: false, canUnselect: true });

  // A directory the test generates (10k files), typed into the entry.
  let entry = $state();
  let dir = $state.raw(null);
  const openDir = () => {
    dir = new DirectoryList({ file: File.newForPath(entry.getProp('text')), attributes: 'standard::display-name' });
    mode = 'dir';
  };

  let strings = $state.raw(null);
  const makeStrings = (n) => {
    strings = new StringList({ strings: Array.from({ length: n }, (_, i) => `Item ${i}`) });
    mode = 'strings';
  };
  let array = $state.raw([]);
  const makeArray = (n) => {
    array = Array.from({ length: n }, (_, i) => ({ id: i, text: `Item ${i}` }));
    mode = 'array';
  };

  let mode = $state('store');
  const model = $derived(
    mode === 'store' ? store : mode === 'chain' ? chain : mode === 'adj' ? adjs : mode === 'sel' ? sel : mode === 'dir' ? dir : mode === 'strings' ? strings : null,
  );
  const items = $derived(model ?? array);
  const itemClass = $derived(mode === 'dir' ? FileInfo : mode === 'adj' ? Adjustment : StringObject);
  // `nItems` is a reactive getter (notify::n-items, R1): no mirror, no items-changed listener.
  const count = $derived(model ? model.nItems : array.length);

  let list = $state(null);
  let selected = $state(-1);
  let activated = $state('-');

  // By the item's class, not by `mode`: rows still showing the previous model re-render once when the
  // mode changes, before the view rebinds them.
  function text(item) {
    if (item instanceof FileInfo) return item.getDisplayName();
    if (item instanceof Adjustment) return `v=${item.value}`;
    if (item instanceof StringObject) return item.string;
    return item?.text ?? '';
  }

  globalThis.__fixtureKeep = [store, base, chain, adjs, sel, filter, sorter];
</script>

<window title="Native models" default-width={480} default-height={600}>
  <box orientation="vertical" spacing="4">
    <box orientation="horizontal" spacing="4">
      <button onclicked={() => (mode = 'store')}>store</button>
      <button onclicked={fill}>fill</button>
      <button onclicked={insert}>insert</button>
      <button onclicked={remove}>remove</button>
      <button onclicked={splice}>splice</button>
      <button onclicked={sortDesc}>sort</button>
      <button onclicked={() => (mode = 'chain')}>chain</button>
      <button onclicked={chainAdd}>chainAdd</button>
      <button onclicked={refilter}>refilter</button>
    </box>
    <box orientation="horizontal" spacing="4">
      <button onclicked={() => (mode = 'adj')}>adj</button>
      <button onclicked={() => (mode = 'sel')}>sel</button>
      <entry bind:this={entry} />
      <button onclicked={openDir}>dir</button>
      <button onclicked={() => makeStrings(10000)}>str10k</button>
      <button onclicked={() => makeStrings(100000)}>str100k</button>
      <button onclicked={() => makeArray(10000)}>arr10k</button>
      <button onclicked={() => makeArray(100000)}>arr100k</button>
    </box>
    <box orientation="horizontal" spacing="4">
      <button onclicked={() => list?.scrollTo(count - 1)}>scrollEnd</button>
      <button onclicked={() => list?.scrollTo(Math.floor(count / 2))}>scrollMid</button>
      <button onclicked={() => list?.scrollTo(0)}>scrollStart</button>
    </box>
    <label>{`mode=${mode} n=${count} selected=${selected} activated=${activated} loading=${dir ? dir.loading : '-'} sel=${sel.selected}`}</label>
    <ListView
      bind:this={list}
      {items}
      {itemClass}
      key={(it) => it.id}
      bind:selected
      vexpand={true}
      min-height={300}
      onactivate={(item, pos) => (activated = `${pos}:${text(item)}`)}
    >
      {#snippet row(item, index)}
        <box orientation="horizontal" spacing="6">
          <label>{index}</label>
          <label>{text(item)}</label>
        </box>
      {/snippet}
    </ListView>
  </box>
</window>
