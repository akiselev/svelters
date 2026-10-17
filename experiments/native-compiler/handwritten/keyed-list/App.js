// Hand-written in the generated-code shape for fixtures/keyed-list/src/App.svelte.
import * as $ from 'svelte/internal/client';
import * as $n from '@svelters/native-runtime';

export default function App($$anchor, $$props) {
  $.push($$props, true);
  let draft = $.state('');
  let nextId = 1;
  let rows = $.state($.proxy([]));

  function add() {
    if ($.get(draft) === '') return;
    $.get(rows).push({ id: nextId++, text: $.get(draft) });
    $.set(draft, '');
  }

  function remove(id) {
    $.set(rows, $.get(rows).filter((r) => r.id !== id), true);
  }

  function reverse() {
    $.set(rows, $.get(rows).toReversed(), true);
  }

  const n1 = $n.element('window', 'present');
  $n.set(n1, 'title', 'Keyed list');
  const n2 = $n.element('box');
  $n.set(n2, 'orientation', 'vertical');
  $n.set(n2, 'spacing', 6);
  const n3 = $n.element('box');
  $n.set(n3, 'orientation', 'horizontal');
  $n.set(n3, 'spacing', 6);
  const n4 = $n.element('entry');
  $n.set(n4, 'placeholder-text', 'New item');
  $.template_effect(() => $n.set(n4, 'text', $.get(draft)));
  // entry.text has a spec binding (changed, arg 0): the one-way prop is tracked, no echo.
  $n.track(n4, 'text', 'changed', 0);
  $n.on(n4, 'changed', (arg) => $.set(draft, arg?.detail ?? arg));
  $n.append($n.slot(n3, null), n4);
  const n5 = $n.element('button');
  $n.on(n5, 'clicked', add);
  $n.set(n5, 'label', 'Add');
  $n.append($n.slot(n3, null), n5);
  const n6 = $n.element('button');
  $n.on(n6, 'clicked', reverse);
  $n.set(n6, 'label', 'Reverse');
  $n.append($n.slot(n3, null), n6);
  $n.append($n.slot(n2, null), n3);
  const n7 = $n.element('scrolled-window');
  const n8 = $n.element('box');
  $n.set(n8, 'orientation', 'vertical');
  $n.set(n8, 'spacing', 2);
  $n.each_block(
    $n.slot(n8, null),
    () => $.get(rows),
    (row) => row.id,
    ($$anchor, row) => {
      const n9 = $n.element('box');
      $n.set(n9, 'orientation', 'horizontal');
      $n.set(n9, 'spacing', 6);
      const n10 = $n.element('label');
      $.template_effect(() => $n.set(n10, 'label', $.get(row).text ?? ''));
      $n.append($n.slot(n9, null), n10);
      const n11 = $n.element('button');
      $n.on(n11, 'clicked', () => remove($.get(row).id));
      $n.set(n11, 'label', 'Remove');
      $n.append($n.slot(n9, null), n11);
      $n.append($$anchor, n9);
    },
    null,
  );
  $n.append($n.slot(n7, null), n8);
  $n.append($n.slot(n2, null), n7);
  $n.append($n.slot(n1, null), n2);
  $n.append($$anchor, n1);
  $.pop();
}
