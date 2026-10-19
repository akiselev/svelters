import * as $ from 'svelte/internal/client';
import * as $n from '@svelters/native-runtime';

export default function input($$anchor, $$props) {
	$.push($$props, true);
	let a = $.state(1);
	let b = $.state(2);
	let plain1 = 0, plain2 = 0;
	let left = $.prop($$props, "left", 7, 0), right = $.prop($$props, "right", 7);
	let arr = $.proxy([]);
	function swap() {
    (($$value) => { var $$array = $.to_array($$value, 2); $.set(a, $$array[0], true); $.set(b, $$array[1], true); })([$.get(b), $.get(a)]);
  }
	function swapExpr() {
    return ((($$value) => { var $$array_1 = $.to_array($$value, 2); $.set(a, $$array_1[0], true); $.set(b, $$array_1[1], true); return $$value; })([$.get(b), $.get(a)]));
  }
	function fromState() {
    ((arr) => { var $$array_2 = $.to_array(arr, 2); $.set(a, $$array_2[0], true); $.set(b, $$array_2[1], true); })(arr);
  }
	function objects() {
    ((($$value) => { $.set(a, $$value.a, true); $.set(b, $.fallback($$value.b, 3), true); })({ a: 4 }));
    ((($$value) => { left($$value.left); right($$value.right); })({ left: 1, right: 2 }));
  }
	function untouched() {
    [plain1, plain2] = [plain2, plain1];
  }
	const $$n1 = $n.element("label");
	$.template_effect(() => $n.set($$n1, "label", `${$.get(a)} ${$.get(b)} ${left()} ${right()}`));
	$n.track($$n1, "label", "notify::label", -1);
	$n.append($$anchor, $$n1);
	$.pop();
}
