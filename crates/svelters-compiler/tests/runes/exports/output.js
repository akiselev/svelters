import * as $ from 'svelte/internal/client';
import * as $n from '@svelters/native-runtime';

export default function input($$anchor, $$props) {
	$.push($$props, true);
	let value = $.prop($$props, "value", 15, 0);
	let count = $.state(0);
	let raw = $.state(1);
	let plain = 1;
	function reset() {
    $.set(count, 0);
    $.set(raw, 2);
    plain = 3;
  }
	const VERSION = '1.0';
	var $$exports = { reset, VERSION, get count() { return $.get(count); }, set count($$value) { $.set(count, $.proxy($$value)); }, get rawValue() { return $.get(raw); }, set rawValue($$value) { $.set(raw, $$value); }, get plain() { return plain; }, set plain($$value) { plain = $$value; }, get value() { return value(); }, set value($$value) { value($$value); } };
	const n1 = $n.element("label");
	$.template_effect(() => $n.set(n1, "label", `${$.get(count)} ${value()}`));
	$n.append($$anchor, n1);
	return $.pop($$exports);
}
