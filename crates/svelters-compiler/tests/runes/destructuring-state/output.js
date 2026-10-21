import * as $ from '@svelters/native-runtime/core';
import * as $n from '@svelters/native-runtime';

export default function Input($$anchor, $$props) {
	$.push($$props, true);
	let tmp = { a: 1, b: 2, c: 3 }, a = $.state($.proxy(tmp.a)), renamed = $.proxy($.fallback(tmp.b, 5)), others = $.proxy($.exclude_from_object(tmp, ["a", "b"]));
	let tmp_1 = [1, 2], $$array = $.derived(() => $.to_array(tmp_1)), x = $.state($.proxy($.get($$array)[0])), y = $.state($.proxy($.fallback($.get($$array)[2], 10))), tail = $.proxy($.get($$array).slice(3));
	let tmp_2 = { p: 1, q: 2 }, p = $.state(tmp_2.p), q = $.state(tmp_2.q);
	let source = $.state($.proxy({ s: 1 }));
	let $$d = $.derived(() => ({ m: $.get(a), n: [$.get(x), $.get(y)] })), $$array_1 = $.derived(() => $.to_array($.get($$d).n, 2)), m = $.derived(() => $.get($$d).m), nA = $.derived(() => $.get($$array_1)[0]), nB = $.derived(() => $.get($$array_1)[1]);
	let $$d_1 = $.derived(() => [renamed, others]), $$array_2 = $.derived(() => $.to_array($.get($$d_1), 2)), first = $.derived(() => $.get($$array_2)[0]), second = $.derived(() => $.get($$array_2)[1]);
	let s = $.derived(() => $.get(source).s);
	function swap() {
    $.set(a, 10);
    (($$value) => { var $$array_3 = $.to_array($$value, 2); $.set(x, $$array_3[0], true); $.set(y, $$array_3[1], true); })([$.get(y), $.get(x)]);
    ((($$value) => { $.set(p, $$value.p); $.set(q, $$value.q); })({ p: $.get(q), q: $.get(p) }));
    $.set(p, 3);
    $.set(source, { s: 2 }, true);
  }
	const $$n1 = $n.element("label");
	$.template_effect(() => $n.set($$n1, "label", `${$.get(a)} ${renamed} ${others.c} ${$.get(x)} ${$.get(y)} ${tail} ${$.get(p)} ${$.get(q)} ${$.get(m)} ${$.get(nA)} ${$.get(nB)} ${$.get(first)} ${$.get(second)} ${$.get(s)}`));
	$n.track($$n1, "label", "notify::label", -1);
	$n.append($$anchor, $$n1);
	$.pop();
}
