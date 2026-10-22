<script>
  // `row` reads nothing from the instance but forwards `clicked`: it stays in the component
  // function (it reads `$$props`); `plain` is hoisted as before.
  import List from './List.svelte';
</script>

{#snippet row(label)}
  <button {label} on:clicked />
{/snippet}

{#snippet plain(label)}
  <label {label} />
{/snippet}

<List {row} {plain} />
