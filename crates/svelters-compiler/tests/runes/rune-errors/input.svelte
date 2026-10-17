<script>
  import { x } from './x.js';
  const c = $state(0);
  let items = [1];
  function f() {
    let inner = $props();
    const nope = $props.id();
    return $state(1);
  }
  let e = () => $effect(() => {});
  let s = $state(1, 2);
  let d = $derived();
  let b = $bindable();
  $effect.tracking(1);
  function g() {
    c = 1;
    console.log(1);
    $inspect.trace();
  }
  function h2() { x = 1; }
</script>

{#each items as item}
  <button onclick={() => (item = 2)}>x</button>
{/each}
