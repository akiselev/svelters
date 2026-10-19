import * as $ from 'svelte/internal/client';
import * as $n from '@svelters/native-runtime';

export default function input($$anchor, $$props) {
	$.push($$props, true);
	let count = $.state(0);
	let snap = $.snapshot($.get(count));
	let eager = $.derived(() => ($.eager(() => ($.get(count)))));
	$.user_effect(() => {
    console.log($.get(count), $.effect_tracking(), $.eager(() => $.pending()));
  });
	$.user_pre_effect(() => {
    $.get(count);
  });
	const stop = $.effect_root(() => {
    $.user_effect(() => {});
    return () => {};
  });
	function later() {
    let local = $.state(1);
    $.update(local);
    return $.get(local);
  }
	for (let i = 0; i < 2; i++) {
    $.set(count, $.get(count) + (i));
  }
	const $$n1 = $n.element("label");
	$.template_effect(() => $n.set($$n1, "label", `${$.get(count)} ${$.get(eager)} ${snap}`));
	$n.track($$n1, "label", "notify::label", -1);
	$n.append($$anchor, $$n1);
	$.pop();
}
