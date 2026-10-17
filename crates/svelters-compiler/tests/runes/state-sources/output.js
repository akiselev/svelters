import * as $ from 'svelte/internal/client';
import * as $n from '@svelters/native-runtime';

export default function input($$anchor, $$props) {
	$.push($$props, true);
	let never = 0;
	let obj = $.proxy({ n: 1 });
	let arr = [1, 2];
	let changed = $.state(0);
	let rawChanged = $.state({ big: 1 });
	let empty = $.state(void 0);
	function go() {
    obj.n++;
    $.set(changed, { fresh: true }, true);
    $.set(changed, $.get(changed) || (5));
    $.set(changed, $.get(changed) * (2 + 1));
    $.set(changed, (1, 2), true);
    $.set(rawChanged, { big: 2 });
    $.set(empty, [], true);
  }
	const n1 = $n.element("label");
	$.template_effect(() => $n.set(n1, "label", `${never} ${obj.n} ${arr.length} ${$.get(changed)} ${$.get(rawChanged).big} ${$.get(empty)}`));
	$n.append($$anchor, n1);
	$.pop();
}
