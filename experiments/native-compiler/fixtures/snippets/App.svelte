<!-- Snippets: local snippets with thunk parameters (rendered in place, re-rendered by the each),
     snippets passed to a component as props (a dynamic one swaps its natives when the function
     changes), the implicit children snippet, an absent optional snippet, spread props. -->
<script>
  import Card from './Card.svelte';

  let items = $state(['a', 'b']);
  let fancy = $state(false);
  let n = $state(1);
  const extra = { title: 'Card', value: 0 };
</script>

{#snippet row(text, i)}
  <label>{i}: {text}</label>
{/snippet}

{#snippet plain(v)}
  <label>plain {v}</label>
{/snippet}

{#snippet bold(v)}
  <button>bold {v}</button>
  <label>!</label>
{/snippet}

<window title="Snippets">
  <box orientation="vertical">
    {#each items as item, i (item)}
      {@render row(item, i)}
    {/each}
    <Card {...extra} body={fancy ? bold : plain} value={n}>
      <button onclicked={() => (fancy = !fancy)}>Swap</button>
      <button onclicked={() => n++}>More</button>
    </Card>
    <button onclicked={() => items.unshift('z')}>Add</button>
  </box>
</window>
