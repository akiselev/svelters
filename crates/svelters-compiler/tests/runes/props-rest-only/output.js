import * as $ from 'svelte/internal/client';
import * as $n from '@svelters/native-runtime';
var rest_excludes = new Set(["$$slots", "$$events", "$$legacy"]);

export default function input($$anchor, $$props) {
	$.push($$props, true);
	let props = $.rest_props($$props, rest_excludes);
	const $$n1 = $n.element("label");
	$.template_effect(() => $n.set($$n1, "label", props.title));
	$n.append($$anchor, $$n1);
	$.pop();
}
