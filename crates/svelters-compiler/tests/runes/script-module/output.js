import * as $ from 'svelte/internal/client';
import * as $n from '@svelters/native-runtime';
import { untrack } from 'svelte';
let instances = $.state(0);
export const shared = $.proxy({ total: 0 });
export function count() {
    return $.get(instances);
  }

export default function input($$anchor, $$props) {
	$.push($$props, true);
	$.update(instances);
	shared.total += 1;
	let local = $.derived(() => ($.get(instances) * 2));
	const $$n1 = $n.element("label");
	$.template_effect(() => $n.set($$n1, "label", `${$.get(instances)} ${shared.total} ${$.get(local)} ${untrack(() => 1)}`));
	$n.track($$n1, "label", "notify::label", -1);
	$n.append($$anchor, $$n1);
	$.pop();
}
