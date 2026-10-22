import '@svelters/native-runtime/core/tracing';
import * as $ from '@svelters/native-runtime/core';
import * as $n from '@svelters/native-runtime';

export default function Input($$anchor, $$props) {
	$.push($$props, true);
	let count = $.tag($.state(0), "count");
	let items = $.tag_proxy($.proxy([]), "items");
	let d = $.tag($.derived(() => ($.get(count) * 2)), "d");
	$.inspect(() => [$.get(count)], (...$$args) => console.log(...$$args), true);
	$.inspect(() => [$.get(count), $.get(count) * 2], (...$$args) => ((type, ...values) => console.info(type, values))(...$$args));
	function f() {
    return $.trace(() => "f (input.svelte:7:2)", () => {
    return $.get(count);
  });}
	const g = async () => {
    return await $.trace(() => ('g'), async () => {
    await 1;
  });};
	function reassign() {
    $.set(count, 1);
  }
	const $$n1 = $n.element("label");
	$.template_effect(() => $n.set($$n1, "label", `${$.get(count)} ${$.get(d)} ${items.length}`));
	$n.track($$n1, "label", "notify::label", -1);
	$n.append($$anchor, $$n1);
	$.pop();
}
