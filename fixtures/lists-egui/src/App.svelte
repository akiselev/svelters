<script>
  // LS1: `<ListView>` (@svelters/lists, the M5 protocol) on egui's `virtual-list`. 2k items; rows
  // are recycled slots bound before the pass (crates/svelters-egui/tests/vlist.rs drives a fling
  // through the list and checks every drawn row shows its own item). The buttons change the items
  // with keyed splices.
  import { ListView } from '@svelters/lists';

  let items = $state.raw(Array.from({ length: 2000 }, (_, i) => ({ id: i + 1, text: `Item ${i + 1}` })));
  let selected = $state(-1);
  let activated = $state('-');
  let list = $state(null);

  const prepend = () => (items = [{ id: 10001, text: 'Item new' }, ...items]);
  const removeFirst = () => (items = items.slice(1));
</script>

<window title="Lists on egui" default-width={360} default-height={420}>
  <box flex-grow={1} gap={4}>
    <box flex-direction="row" gap={4}>
      <button onclicked={prepend}>prepend</button>
      <button onclicked={removeFirst}>removeFirst</button>
      <button onclicked={() => list?.scrollTo(items.length - 1)}>scrollEnd</button>
      <label>{`selected=${selected} activated=${activated}`}</label>
    </box>
    <ListView
      bind:this={list}
      {items}
      key={(it) => it.id}
      bind:selected
      row-height={24}
      flex-grow={1}
      onactivate={(item, pos) => (activated = `${pos}:${item.id}`)}
    >
      {#snippet row(item, index)}
        <box flex-direction="row" gap={6}>
          <label>{item.text}</label>
          <label>{`#${index}`}</label>
        </box>
      {/snippet}
    </ListView>
  </box>
</window>
