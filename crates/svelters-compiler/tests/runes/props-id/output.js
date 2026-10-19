import * as $ from 'svelte/internal/client';
import * as $n from '@svelters/native-runtime';
var rest_excludes = new Set(["$$slots", "$$events", "$$legacy", "a", "b", "c", "d", "e", "f", "g-h"]);

export default function input($$anchor, $$props) {
	$.push($$props, true);
	const uid = $.props_id();
	let b = $.prop($$props, "b", 7, 1), c = $.prop($$props, "c", 19, compute), d = $.prop($$props, "d", 19, () => [1]), e = $.prop($$props, "e", 31, () => $.proxy({})), rest = $.rest_props($$props, rest_excludes);
	function compute() {
    return 2;
  }
	function touch() {
    b(2);
    e(e().x = 1, true);
  }
	const $$n1 = $n.element("label");
	$.template_effect(() => $n.set($$n1, "label", `${uid} ${$$props.a} ${b()} ${c()} ${d()} ${e()} ${$$props.f} ${$$props["g-h"]} ${rest.z}`));
	$n.track($$n1, "label", "notify::label", -1);
	$n.append($$anchor, $$n1);
	$.pop();
}
