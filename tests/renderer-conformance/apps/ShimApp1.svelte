<script>
  let count = $state(0);
  let items = $state([{id:1,name:'a'},{id:2,name:'b'}]);
  let { title } = $props();
</script>
<window title={title}>
  <button class="btn" disabled={count > 3} onclick={() => count++}>Clicked {count}</button>
  {#if count % 2}
    <label>odd</label>
  {:else}
    <label>even</label>
  {/if}
  {#each items as item (item.id)}
    <row data-id={item.id}>{item.name}</row>
  {/each}
  <entry oninput={(e) => console.log(e)} value={title} />
</window>
