import * as $ from '@svelters/native-runtime/core';
import * as $n from '@svelters/native-runtime';

export default function input($$anchor, $$props) {
	$.push($$props, true);
	let count = $.state(0);
	let items = $.proxy([]);
	let d = $.derived(() => ($.get(count) * 2));
	void 0;
	void 0;
	function f() {
    
    return $.get(count);
  }
	const g = async () => {
    
    await 1;
  };
	function reassign() {
    $.set(count, 1);
  }
	const $$n1 = $n.element("label");
	$.template_effect(() => $n.set($$n1, "label", `${$.get(count)} ${$.get(d)} ${items.length}`));
	$n.track($$n1, "label", "notify::label", -1);
	$n.append($$anchor, $$n1);
	$.pop();
}
