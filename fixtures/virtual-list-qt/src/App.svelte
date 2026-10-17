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

<main-window window-title="Virtual list">
  <box-layout direction="top-to-bottom" spacing="6">
    <box-layout direction="left-to-right" spacing="6">
      <push-button onclicked={add}>Add</push-button>
      <push-button onclicked={prepend}>Prepend</push-button>
      <push-button onclicked={removeSecond}>Remove</push-button>
      <push-button onclicked={reverse}>Reverse</push-button>
      <push-button onclicked={rename}>Rename</push-button>
      <label>{selected}</label>
    </box-layout>
    <ListView {items} key={(it) => it.id} bind:selected>
      {#snippet row(item, index)}
        <box-layout direction="left-to-right" spacing="6">
          <label>{index}</label>
          <label>{item.text}</label>
        </box-layout>
      {/snippet}
    </ListView>
  </box-layout>
</main-window>
