// Hand-written in the generated-code shape for fixtures/counter/src/App.svelte.
import * as $ from '@svelters/native-runtime/core';
import * as $n from '@svelters/native-runtime';

export default function App($$anchor, $$props) {
  $.push($$props, true);
  let count = $.state(0);
  const n1 = $n.element('window', 'present');
  $n.set(n1, 'title', 'Counter');
  const n2 = $n.element('box');
  $n.set(n2, 'orientation', 'vertical');
  $n.set(n2, 'spacing', 8);
  const n3 = $n.element('label');
  $.template_effect(() => $n.set(n3, 'label', `Count: ${$.get(count) ?? ''}`));
  $n.append($n.slot(n2, null), n3);
  const n4 = $n.element('button');
  $n.on(n4, 'clicked', () => $.update(count));
  $n.set(n4, 'label', 'Increment');
  $n.append($n.slot(n2, null), n4);
  $n.append($n.slot(n1, null), n2);
  $n.append($$anchor, n1);
  $.pop();
}
