<script>
  let n = $state(1);
  let p = $derived.by(() => { const v = n; return new Promise((r, j) => setTimeout(() => (v < 0 ? j(new Error('neg')) : r(v * 10)), 5)); });
  export function set(v) { n = v; }
</script>
<box>{#await p}<spinner/>{:then v}<done>{v}</done>{:catch e}<err>{e.message}</err>{/await}</box>
