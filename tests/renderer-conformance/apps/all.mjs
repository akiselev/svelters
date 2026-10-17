import * as R from '#renderer';
import { report } from '../harness.mjs';
import Counter from './Counter.svelte';
import If from './If.svelte';
import Each from './Each.svelte';
import Snippet from './Snippet.svelte';
import Key from './Key.svelte';
import Await from './Await.svelte';
import BindThis from './BindThis.svelte';
import Props from './Props.svelte';
import Events from './Events.svelte';
import Values from './Values.svelte';
import Directives from './Directives.svelte';
import Attach from './Attach.svelte';
import Form from './Form.svelte';
import ShimApp1 from './ShimApp1.svelte';
import ShimApp2 from './ShimApp2.svelte';

const only = globalThis.process?.env?.ONLY ?? globalThis.ONLY;
const want = (n) => !only || only.split(',').includes(n);

if (want('Counter'))
	await report(R, 'Counter', Counter, async (c) => {
		const b = c.find('button');
		for (let i = 0; i < 4; i++) await c.step(`click ${i + 1}`, () => c.log('emit returned', R.emit(b, 'click', { type: 'click' }).map(String)));
	});

if (want('If'))
	await report(R, 'If', If, async (c) => {
		for (let i = 0; i < 3; i++) await c.step(`inc -> ${i + 1}`, () => c.app.inc());
	});

if (want('Each'))
	await report(R, 'Each', Each, async (c) => {
		await c.step('reverse', () => c.app.reverse());
		await c.step('insert at 1', () => c.app.insert());
		await c.step('remove first', () => c.app.removeFirst());
		await c.step('rename first', () => c.app.rename());
	});

if (want('Snippet')) await report(R, 'Snippet', Snippet, async (c) => c.step('inc', () => c.app.inc()));

if (want('Key'))
	await report(R, 'Key', Key, async (c) => {
		const before = c.find('k');
		await c.step('inc', () => c.app.inc());
		c.log('k node replaced:', before !== c.find('k'));
	});

if (want('Await'))
	await report(R, 'Await', Await, async (c) => {
		await c.step('resolved', c.settle);
		await c.step('set(2) pending', () => c.app.set(2));
		await c.step('resolved 2', c.settle);
		await c.step('set(-1) -> reject', async () => (c.app.set(-1), c.flushSync(), await c.settle()));
	});

if (want('BindThis'))
	await report(R, 'BindThis', BindThis, async (c) => {
		c.log('bound is entry node:', c.app.get() === c.find('entry'), R.label(c.app.get()));
		await c.step('toggle off', () => c.app.toggle());
		c.log('after removal bound =', String(c.app.get()));
	});

if (want('Props'))
	await report(R, 'Props', Props, async (c) => {
		await c.step('parent inc', () => c.app.inc());
		await c.step('child press', () => R.emit(c.find('button'), 'click', { type: 'click' }));
		c.log('parent received', c.app.got_());
	});

if (want('Events'))
	await report(R, 'Events', Events, async (c) => {
		const w = c.target.children?.[0] ?? c.target._c[0];
		c.log('close-request(widget, 7) ->', R.emit(w, 'close-request', { widget: 1 }, 7).map(String));
		const ev = { type: 'keypress', prevented: false, preventDefault() { this.prevented = true; } };
		c.log('keypress ->', R.emit(w, 'keypress', ev).map(String), 'prevented', ev.prevented);
		const ev2 = { type: 'click', prevented: false, preventDefault() { this.prevented = true; } };
		c.log('click ->', R.emit(w, 'click', ev2).map(String), 'prevented', ev2.prevented);
		c.log('activate ->', R.emit(w, 'activate').map(String));
		c.log('handler saw', c.app.getSeen());
		c.log('listener keys on node', Object.keys(w.listeners ?? w._l ?? {}));
	});

if (want('Values')) await report(R, 'Values', Values, async (c) => c.step('inc', () => c.app.inc()));

if (want('Directives')) await report(R, 'Directives', Directives, async (c) => c.step('toggle', () => c.app.toggle()));

if (want('Attach'))
	await report(R, 'Attach', Attach, async (c) => {
		await c.step('inc', () => c.app.inc());
		c.log('log', c.app.getLog());
	}).then(() => 0);

if (want('Form'))
	await report(R, 'Form (bind:value via $bindable wrapper)', Form, async (c) => {
		const e = c.find('entry');
		await c.step('native typed "Bob"', () => R.emit(e, 'changed', 'Bob'));
		c.log('parent name =', c.app.getName());
		await c.step('parent setName("Cy")', () => c.app.setName('Cy'));
	});

if (want('ShimApp1'))
	await report(
		R,
		'ShimApp1 (compile/scripts/App.svelte)',
		ShimApp1,
		async (c) => {
			const b = c.find('button');
			for (let i = 0; i < 5; i++) R.emit(b, 'click', { type: 'click' }), c.flushSync();
			await c.step('after 5 clicks');
		},
		{ title: 'hi' }
	);

if (want('ShimApp2'))
	await report(R, 'ShimApp2 (compile/scripts/App2.svelte)', ShimApp2, async (c) => {
		await c.step('settle', c.settle);
		await c.step('activate', () => R.emit(c.find('box'), 'activate', { type: 'activate' }));
		await c.step('settle', c.settle);
	});
