<!-- Deliberately invalid: snippet/block/spread validations (each line exercises one; the destructuring ones compile since CF1 and stay as positive cases). -->
<script>
  let props = $state({ label: 'x' });
  let items = $state([1, 2]);
  let p = $state(Promise.resolve({ a: 1 }));
</script>

{#snippet bar()}
  <label>not a menu bar</label>
{/snippet}

{#snippet pair()}
  <button>one</button>
  <statusbar />
{/snippet}

{#snippet destructured({ a })}
  <label>{a}</label>
{/snippet}

<sv-slot name="central"></sv-slot>

<window title="Diagnostics">
  <box orientation="vertical">
    {@const misplaced = 1}
    <button {...props}>Spread</button>
    <label>{#each items as i}{i}{/each}</label>
    {#each items as i}
      {@const { x } = { x: i }}
      <label>{x}</label>
    {/each}
    {#await p then { a }}
      <label>{a}</label>
    {/await}
  </box>
  <sv-slot name="menubar">{@render bar()}</sv-slot>
  {@render pair()}
</window>
