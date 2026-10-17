<script>
  let n = $state(0);
  let p = $derived(new Promise((r) => setTimeout(() => r(n * 10), 5)));
  let el = $state();
  $effect(() => { console.log('effect n=', n, 'el=', el?.localName); });
</script>
{#snippet cell(x)}<cell v={x}>{x}</cell>{/snippet}
<box {@attach (n) => { el = n; }} onactivate={() => n++} style:color="red" class:on={n > 0}>
  {@render cell(n)}
  {#key n}<k>{n}</k>{/key}
  {#await p}<spinner/>{:then v}<done>{v}</done>{/await}
</box>
