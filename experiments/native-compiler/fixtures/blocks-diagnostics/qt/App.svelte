<!-- Qt-vocabulary copy of ../App.svelte (the qt compile target). -->
<!-- Deliberately invalid: snippet/block/spread validations (each line exercises one). -->
<script>
  let props = $state({ label: 'x' });
  let items = $state([1, 2]);
  let p = $state(Promise.resolve({ a: 1 }));
</script>

{#snippet bar()}
  <label>not a menu bar</label>
{/snippet}

{#snippet pair()}
  <push-button>one</push-button>
  <status-bar />
{/snippet}

{#snippet destructured({ a })}
  <label>{a}</label>
{/snippet}

<sv-slot name="central"></sv-slot>

<main-window window-title="Diagnostics">
  <box-layout direction="top-to-bottom">
    {@const misplaced = 1}
    <push-button {...props}>Spread</push-button>
    <label>{#each items as i}{i}{/each}</label>
    {#each items as i}
      {@const { x } = { x: i }}
      <label>{x}</label>
    {/each}
    {#await p then { a }}
      <label>{a}</label>
    {/await}
  </box-layout>
  <sv-slot name="menubar">{@render bar()}</sv-slot>
  {@render pair()}
</main-window>
