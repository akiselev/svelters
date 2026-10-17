<!-- Qt-vocabulary copy of ../App.svelte (the qt compile target). -->
<!-- .svelte.js module state + <script module> state, read from the template. -->
<script module>
  let mounted = $state(0);
  export const label = 'Module';
</script>

<script>
  import { increment, current, history, Doubler } from '../counter.svelte.js';
  mounted++;
  const doubler = new Doubler();
  let last = $derived(history.at(-1) ?? 'none');
</script>

<main-window window-title={label}>
  <box-layout direction="top-to-bottom" spacing="4">
    <label>{current()} (last {last}, {history.length} steps) x{doubler.value} mounted {mounted}</label>
    <push-button onclicked={() => increment()}>+1</push-button>
    <push-button onclicked={() => increment(5)}>+5</push-button>
    <push-button onclicked={() => doubler.bump()}>Factor</push-button>
  </box-layout>
</main-window>
