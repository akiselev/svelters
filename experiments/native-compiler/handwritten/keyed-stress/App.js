// Hand-written in the generated-code shape for fixtures/keyed-stress/src/App.svelte.
import * as $ from 'svelte/internal/client';
import * as $n from '@svelters/native-runtime';
import { onMount } from 'svelte';

export default function App($$anchor, $$props) {
  $.push($$props, true);
  let rows = $.state($.proxy([1, 2, 3, 4, 5, 6]));
  let next = 7;
  let expected = $.state('');

  const steps = [
    () => $.set(rows, $.get(rows).toReversed(), true),
    () => $.get(rows).splice(2, 0, next++),
    () => $.set(rows, [next++, ...$.get(rows)], true),
    () => $.set(rows, [...$.get(rows), next++], true),
    () => $.set(rows, $.get(rows).filter((_, i) => i !== 0 && i !== 4), true),
    () => $.set(rows, $.get(rows).toSorted((a, b) => ((a * 7) % 11) - ((b * 7) % 11)), true),
    () => $.set(rows, $.get(rows).toReversed(), true),
    () => $.set(rows, [...$.get(rows).slice(3), ...$.get(rows).slice(0, 3)], true),
    () => $.set(rows, $.get(rows).filter((r) => r % 2 === 0), true),
    () => $.set(rows, [...$.get(rows), 100, 101].reverse(), true),
  ];

  onMount(() => {
    let i = 0;
    const tick = () => {
      if (i < steps.length) {
        steps[i++]();
        setTimeout(tick, 1);
      } else {
        $.set(expected, $.get(rows).join(','));
      }
    };
    setTimeout(tick, 1);
  });

  const n1 = $n.element('window', 'present');
  $n.set(n1, 'title', 'Keyed stress');
  const n2 = $n.element('box');
  $n.set(n2, 'orientation', 'vertical');
  $n.set(n2, 'spacing', 2);
  const n3 = $n.element('label');
  $.template_effect(() => $n.set(n3, 'label', $.get(expected) ?? ''));
  $n.append($n.slot(n2, null), n3);
  const n4 = $n.element('box');
  $n.set(n4, 'orientation', 'vertical');
  $n.set(n4, 'spacing', 2);
  $n.each_block(
    $n.slot(n4, null),
    () => $.get(rows),
    (id) => id,
    ($$anchor, id) => {
      const n5 = $n.element('label');
      $.template_effect(() => $n.set(n5, 'label', `${$.get(id) ?? ''}`));
      $n.append($$anchor, n5);
    },
    null,
  );
  $n.append($n.slot(n2, null), n4);
  $n.append($n.slot(n1, null), n2);
  $n.append($$anchor, n1);
  $.pop();
}
