// Shared scenario runner. Works for both backends via the '#renderer' adapter (R).
import { mount, unmount, flushSync } from 'svelte';

const p = (s) => (typeof print === 'function' ? print(s) : console.log(s)); // qjs has print()
const settle = () => new Promise((r) => setTimeout(r, 20));

export async function report(R, name, App, scenario, props = {}) {
	const target = R.root('window');
	let seen = R.ops.length;
	const flushOps = () => {
		const out = R.ops.slice(seen);
		seen = R.ops.length;
		return out;
	};
	const ctx = {
		R,
		target,
		app: null,
		flushSync,
		settle,
		find: (t) => R.find(target, t),
		findAll: (t) => R.findAll(target, t),
		log: (...a) => p('  > ' + a.map((x) => (typeof x === 'string' ? x : JSON.stringify(x))).join(' ')),
		async step(title, fn) {
			if (fn) await fn();
			flushSync();
			const o = flushOps();
			const c = R.count(target);
			p(`--- ${title}  [ops ${o.length}; tree el=${c.element} text=${c.text} comment=${c.comment}]`);
			for (const l of o) p('    op: ' + l);
			p(R.dump(target).replace(/^/gm, '    | ').trimEnd());
		}
	};
	p(`===== ${name}`);
	await ctx.step('mount', () => {
		ctx.app = mount(App, { target, props, ...(R.renderer ? { renderer: R.renderer } : {}) });
	});
	await scenario(ctx);
	await ctx.step('unmount', () => unmount(ctx.app));
	const left = R.count(target);
	p(`leftover after unmount: ${JSON.stringify(left)}; reads: ${JSON.stringify(R.reads)}`);
}
