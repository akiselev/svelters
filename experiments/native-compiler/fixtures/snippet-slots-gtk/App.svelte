<!-- Snippets as named-slot content. A component renders a snippet prop into a native slot of its
     own element (<sv-slot name="titlebar">) and removes it when the prop goes away; a local
     snippet rendered into the headerbar's `end` slot is checked against the GTK spec at the
     render site. Qt has no headerbar. -->
<script>
  import Frame from './Frame.svelte';

  let custom = $state(true);
  let count = $state(0);
</script>

{#snippet actions(n)}
  <button>Undo {n}</button>
  <button>Redo</button>
{/snippet}

{#snippet bar()}
  <header-bar>
    <button onclicked={() => count++}>Add</button>
    <sv-slot name="end">{@render actions(count)}</sv-slot>
  </header-bar>
{/snippet}

<Frame title="Slots" titlebar={custom ? bar : null}>
  <box orientation="vertical">
    <label>{count} items</label>
    <button onclicked={() => (custom = !custom)}>Toggle</button>
  </box>
</Frame>
