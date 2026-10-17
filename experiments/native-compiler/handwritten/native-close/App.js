// Hand-written in the generated-code shape for fixtures/native-close/src/App.svelte.
import * as $ from 'svelte/internal/client';
import * as $n from '@svelters/native-runtime';

export default function App($$anchor, $$props) {
  $.push($$props, true);
  let open = $.state(true);
  const n1 = $n.element('window', 'present');
  $n.set(n1, 'title', 'Main');
  const n2 = $n.element('box');
  $n.set(n2, 'orientation', 'vertical');
  $n.set(n2, 'spacing', 8);
  const n3 = $n.element('label');
  $.template_effect(() => $n.set(n3, 'label', $.get(open) ? 'child open' : 'child closed'));
  $n.append($n.slot(n2, null), n3);
  const n4 = $n.element('button');
  $n.on(n4, 'clicked', () => $.set(open, !$.get(open)));
  $n.set(n4, 'label', 'Toggle child');
  $n.append($n.slot(n2, null), n4);
  $n.append($n.slot(n1, null), n2);
  $n.append($$anchor, n1);
  $n.if_block(
    $$anchor,
    () => $.get(open),
    ($$anchor) => {
      const n5 = $n.element('window', 'present');
      $n.set(n5, 'title', 'Child');
      const n6 = $n.element('label');
      $n.set(n6, 'label', 'Child body');
      $n.append($n.slot(n5, null), n6);
      $n.append($$anchor, n5);
    },
    null,
  );
  $.pop();
}
