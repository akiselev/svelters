// Hand-written in the generated-code shape for fixtures/timers/src/App.svelte.
import * as $ from '@svelters/native-runtime/core';
import * as $n from '@svelters/native-runtime';
import { onMount } from 'svelte';

export default function App($$anchor, $$props) {
  $.push($$props, true);
  let status = $.state('waiting');
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

  onMount(() => {
    (async () => {
      await sleep(10);
      $.set(status, 'timer 1');
      await Promise.resolve();
      await sleep(5);
      $.set(status, 'timer 2');
      await sleep(20);
      $.set(status, 'done');
    })();
  });

  const n1 = $n.element('window', 'present');
  $n.set(n1, 'title', 'Timers');
  const n2 = $n.element('label');
  $n.append($n.slot(n1, null), n2);
  $n.append($$anchor, n1);
  // the fragment's template effect (Svelte's order)
  $.template_effect(() => $n.set(n2, 'label', $.get(status) ?? ''));
  $.pop();
}
