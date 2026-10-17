<script>
  // Row-slot semantics of <ListView> (virtualized list), run on both pipelines: rows are recycled
  // slots, so state local to a row component belongs to the slot and survives a rebind to another
  // item; the row reads the list's context; bound rows follow splices and item replacements without
  // a rebind; bind:selected, onactivate, scrollTo through bind:this, teardown of every slot when the
  // list goes away. script.json drives the toolkit callbacks synchronously (call_sync),
  // script.deferred.json as deferred deliveries (the toolkit called while JS was busy).
  import { setContext } from 'svelte';
  import { ListView } from '@svelters/lists';
  import Row from './Row.svelte';

  setContext('tag', 'ctx');
  let items = $state.raw(Array.from({ length: 8 }, (_, i) => ({ id: i + 1, text: `Item ${i + 1}` })));
  let selected = $state(-1);
  let activated = $state('-');
  let show = $state(true);
  let list = $state(null);

  const removeFirst = () => (items = items.slice(1));
  const rename = () => (items = items.map((it) => (it.id === 3 || it.id === 6 ? { ...it, text: `${it.text}!` } : it)));
</script>

<main-window window-title="Lists recycle">
  <box-layout direction="top-to-bottom" spacing="4">
    <box-layout direction="left-to-right" spacing="4">
      <push-button onclicked={removeFirst}>removeFirst</push-button>
      <push-button onclicked={rename}>rename</push-button>
      <push-button onclicked={() => (selected = 2)}>select2</push-button>
      <push-button onclicked={() => list?.scrollTo(items.length - 1)}>scrollEnd</push-button>
      <push-button onclicked={() => (show = !show)}>toggle</push-button>
      <label>{`selected=${selected} activated=${activated}`}</label>
    </box-layout>
    {#if show}
      <ListView
        bind:this={list}
        {items}
        key={(it) => it.id}
        bind:selected
        min-height={200}
        onactivate={(item, pos) => (activated = `${pos}:${item.id}`)}
      >
        {#snippet row(item, index)}
          <Row {item} {index} />
        {/snippet}
      </ListView>
    {/if}
  </box-layout>
</main-window>
