import * as $ from '@svelters/native-runtime/core';
import * as $n from '@svelters/native-runtime';
import { writable } from 'svelte/store';

export default function Input($$anchor, $$props) {
	$.push($$props, true);
	const $count = () => $.store_get(count, "$count", $$stores);
	const $user = () => $.store_get(user, "$user", $$stores);
	const $theme = () => $.store_get($$props.theme, "$theme", $$stores);
	const [$$stores, $$cleanup] = $.setup_stores();
	const count = writable(0);
	const user = writable({ name: 'Ada', tags: [] });
	function bump() {
    $.update_store(count, $count());
    $.update_pre_store(count, $count());
    $.store_set(count, $count() + (2));
    $.store_set(count, $count() * 2);
    $.store_mutate(user, $.untrack($user).name = 'Grace', $.untrack($user));
    $user().tags.push('x');
    $.store_mutate(user, $.untrack($user).tags[0] = 'y', $.untrack($user));
  }
	const $$n1 = $n.element("label");
	$n.track($$n1, "label", "notify::label", -1);
	$n.append($$anchor, $$n1);
	const $$n2 = $n.element("entry");
	$n.append($$anchor, $$n2);
	$.template_effect(() => $n.set($$n1, "label", `${$count()} ${$user().name} ${$theme()}`));
	$n.bind_prop($$n2, "text", () => $user().name, ($$value) => { $.store_mutate(user, $.untrack($user).name = $$value, $.untrack($user)); }, "changed", 0);
	$.pop();
	$$cleanup();
}
