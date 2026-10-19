// Hand-written in the generated-code shape for fixtures/if-toggle/src/App.svelte.
import * as $ from '@svelters/native-runtime/core';
import * as $n from '@svelters/native-runtime';

export default function App($$anchor, $$props) {
  $.push($$props, true);
  let show = $.state(false);
  const n1 = $n.element('window', 'present');
  $n.set(n1, 'title', 'If toggle');
  const n2 = $n.element('header-bar');
  $n.set(n2, 'title', 'Toggle');
  $n.append($n.slot(n1, 'titlebar'), n2);
  const n3 = $n.element('box');
  $n.set(n3, 'orientation', 'vertical');
  $n.set(n3, 'spacing', 8);
  const n4 = $n.element('button');
  $n.on(n4, 'clicked', () => $.set(show, !$.get(show)));
  $n.set(n4, 'label', 'Toggle');
  $n.append($n.slot(n3, null), n4);
  $n.if_block(
    $n.slot(n3, null),
    () => $.get(show),
    ($$anchor) => {
      const n5 = $n.element('label');
      $n.set(n5, 'label', 'Shown');
      $n.append($$anchor, n5);
    },
    ($$anchor) => {
      const n6 = $n.element('label');
      $n.set(n6, 'label', 'Hidden');
      $n.append($$anchor, n6);
    },
  );
  $n.append($n.slot(n1, null), n3);
  $n.append($$anchor, n1);
  $.pop();
}
