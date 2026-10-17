<!-- Qt-vocabulary copy of ../App.svelte (the qt compile target). -->
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
  <push-button>bold {v}</push-button>
  <label>!</label>
{/snippet}

<main-window window-title="Snippets">
  <box-layout direction="top-to-bottom">
    {#each items as item, i (item)}
      {@render row(item, i)}
    {/each}
    <Card {...extra} body={fancy ? bold : plain} value={n}>
      <push-button onclicked={() => (fancy = !fancy)}>Swap</push-button>
      <push-button onclicked={() => n++}>More</push-button>
    </Card>
    <push-button onclicked={() => items.unshift('z')}>Add</push-button>
  </box-layout>
</main-window>
