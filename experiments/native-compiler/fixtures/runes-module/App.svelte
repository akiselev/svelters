<!-- .svelte.js module state + <script module> state, read from the template. -->
<script module>
  let mounted = $state(0);
  export const label = 'Module';
</script>

<script>
  import { increment, current, history, Doubler } from './counter.svelte.js';
  mounted++;
  const doubler = new Doubler();
  let last = $derived(history.at(-1) ?? 'none');
</script>

<window title={label}>
  <box orientation="vertical" spacing="4">
    <label>{current()} (last {last}, {history.length} steps) x{doubler.value} mounted {mounted}</label>
    <button onclicked={() => increment()}>+1</button>
    <button onclicked={() => increment(5)}>+5</button>
    <button onclicked={() => doubler.bump()}>Factor</button>
  </box>
</window>
