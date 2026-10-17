<script>
  import { ListView } from '@svelters/lists';

  let items = $state(Array.from({ length: 20 }, (_, i) => ({ id: i, text: `Row ${i}` })));
  let selected = $state(-1);
  let next = 20;

  const add = () => items.push({ id: next++, text: `Row ${next - 1}` });
  const prepend = () => (items = [{ id: next++, text: `Row ${next - 1}` }, ...items]);
  const removeSecond = () => (items = items.filter((_, i) => i !== 1));
  const reverse = () => (items = items.toReversed());
  const rename = () => (items = items.map((it) => (it.id === 3 ? { ...it, text: 'renamed' } : it)));
</script>

<window title="Virtual list">
  <box orientation="vertical" spacing="6">
    <box orientation="horizontal" spacing="6">
      <button onclick={add}>Add</button>
      <button onclick={prepend}>Prepend</button>
      <button onclick={removeSecond}>Remove</button>
      <button onclick={reverse}>Reverse</button>
      <button onclick={rename}>Rename</button>
      <label>{selected}</label>
    </box>
    <ListView {items} key={(it) => it.id} bind:selected>
      {#snippet row(item, index)}
        <box orientation="horizontal" spacing="6">
          <label>{index}</label>
          <label>{item.text}</label>
        </box>
      {/snippet}
    </ListView>
  </box>
</window>
