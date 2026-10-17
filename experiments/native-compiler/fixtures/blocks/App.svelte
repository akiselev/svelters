<!-- {#key} (destroy + re-create the natives), {#await} (pending/then/catch with Svelte's
     resolution order; a non-promise and a stale promise), {@const} (template-scoped deriveds),
     text content holding an {#if} (one conditional label text). -->
<script>
  let id = $state(1);
  let ok = $state(true);
  let items = $state([
    { name: 'x', qty: 2 },
    { name: 'y', qty: 3 },
  ]);
  const load = (v, fail, ms = 10) =>
    new Promise((resolve, reject) => setTimeout(() => (fail ? reject(new Error(`failed ${v}`)) : resolve(`data ${v}`)), ms));
  let promise = $state(load(1, false));
</script>

<window title="Blocks">
  <box orientation="vertical">
    {#key id}
      <entry text="fresh {id}" />
    {/key}
    <label>{#if ok}on{:else if id > 3}late{:else}off{/if} / {id}</label>
    {#each items as item}
      {@const total = item.qty * 10}
      {@const label = `${item.name}: ${total}`}
      <label>{label}</label>
    {/each}
    {#await promise}
      <label>loading</label>
    {:then v}
      <label>{v}</label>
    {:catch e}
      <label>{e.message}</label>
    {/await}
    {#await 42 then v}
      <label>sync {v}</label>
    {/await}
    <button onclick={() => { id++; ok = !ok; }}>Next</button>
    <button onclick={() => (promise = load(id, id % 2 === 0))}>Reload</button>
    <button onclick={() => { load(0, false, 50); promise = load(id, false, 5); promise = Promise.resolve('instant'); }}>Instant</button>
    <button onclick={() => items[0].qty++}>Qty</button>
  </box>
</window>
