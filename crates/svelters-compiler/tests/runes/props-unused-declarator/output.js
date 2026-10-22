import * as $ from '@svelters/native-runtime/core';
import * as $n from '@svelters/native-runtime';

export default function Input($$anchor, $$props) {
	$.push($$props, true);
	let other = 1;
	const $$n1 = $n.element("label");
	$n.track($$n1, "label", "notify::label", -1);
	$n.append($$anchor, $$n1);
	$.template_effect(() => $n.set($$n1, "label", `${$$props.title} ${other}`));
	$.pop();
}
