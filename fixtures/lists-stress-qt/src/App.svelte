<script>
  import { ListView } from '@svelters/lists';

  // Qt twin of fixtures/lists-stress (Qt vocabulary). Driven by examples/counter-qt/tests/lists_e2e.rs:
  // it clicks the buttons by label and watches the native model, the row slots and the status label.
  // Big lists use $state.raw: no per-row proxies, updates replace the array.
  let items = $state.raw([]);
  let selected = $state(-1);
  let activated = $state('-');
  let list = $state(null);
  let next = 0;
  let show = $state(true);

  const make = (n) => Array.from({ length: n }, () => ({ id: next, text: `Item ${next++}` }));
  const fill = (n) => {
    next = 0;
    items = make(n);
  };
  const append = () => (items = [...items, ...make(1)]);
  const prepend = () => (items = [...make(1), ...items]);
  const removeFirst = () => (items = items.slice(1));
  const removeMiddle = () => (items = items.filter((_, i) => i !== 5 && i !== 6 && i !== 7));
  const reverse = () => (items = items.toReversed());
  const rename = () => (items = items.map((it) => (it.id % 10 === 0 ? { ...it, text: `${it.text}!` } : it)));
  const sortDesc = () => (items = items.toSorted((a, b) => b.id - a.id));
  const odd = () => (items = items.filter((it) => it.id % 2 === 1));
  const clear = () => (items = []);
  const moveFirstToEnd = () => (items = [...items.slice(1), items[0]]);
</script>

<main-window window-title="Lists stress">
  <box-layout direction="top-to-bottom" spacing="4">
    <box-layout direction="left-to-right" spacing="4">
      <push-button onclicked={() => fill(1000)}>fill1k</push-button>
      <push-button onclicked={() => fill(10000)}>fill10k</push-button>
      <push-button onclicked={() => fill(100000)}>fill100k</push-button>
      <push-button onclicked={append}>append</push-button>
      <push-button onclicked={prepend}>prepend</push-button>
      <push-button onclicked={removeFirst}>removeFirst</push-button>
      <push-button onclicked={removeMiddle}>removeMiddle</push-button>
    </box-layout>
    <box-layout direction="left-to-right" spacing="4">
      <push-button onclicked={reverse}>reverse</push-button>
      <push-button onclicked={rename}>rename</push-button>
      <push-button onclicked={sortDesc}>sortDesc</push-button>
      <push-button onclicked={odd}>odd</push-button>
      <push-button onclicked={clear}>clear</push-button>
      <push-button onclicked={moveFirstToEnd}>moveFirst</push-button>
      <push-button onclicked={() => list?.scrollTo(items.length - 1)}>scrollEnd</push-button>
      <push-button onclicked={() => list?.scrollTo(0)}>scrollStart</push-button>
      <push-button onclicked={() => list?.scrollTo(Math.floor(items.length / 2))}>scrollMid</push-button>
      <push-button onclicked={() => (show = !show)}>toggle</push-button>
    </box-layout>
    <label>{`items=${items.length} selected=${selected} activated=${activated}`}</label>
    {#if show}
    <ListView
      bind:this={list}
      {items}
      key={(it) => it.id}
      bind:selected
      vexpand={true}
      min-height={300}
      onactivate={(item, pos) => (activated = `${pos}:${item.id}`)}
    >
      {#snippet row(item, index)}
        <box-layout direction="left-to-right" spacing="6">
          <label>{index}</label>
          <label>{item.text}</label>
        </box-layout>
      {/snippet}
    </ListView>
    {/if}
  </box-layout>
</main-window>
