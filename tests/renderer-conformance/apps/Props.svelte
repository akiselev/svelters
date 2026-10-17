<script>
  import Child from './Child.svelte';
  let c = $state(5);
  let got = $state([]);
  export function inc() { c++; }
  export function got_() { return got; }
</script>
<box>
  <Child label="hello" count={c} onpress={(v) => got.push(v)}>inner {c}</Child>
</box>
