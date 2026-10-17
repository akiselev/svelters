import * as $ from 'svelte/internal/client';
import * as $n from '@svelters/native-runtime';

export default function input($$anchor, $$props) {
	$.push($$props, true);
	let other = 1;
	const $$n1 = $n.element("label");
	$.template_effect(() => $n.set($$n1, "label", `${$$props.title} ${other}`));
	$n.append($$anchor, $$n1);
	$.pop();
}
