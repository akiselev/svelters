const accepts = {};
globalThis.__hot = new Proxy({}, { get: (_, file) => ({ accept: (cb) => (accepts[file] = cb) }) });
const R = await import('#renderer');
const { report } = await import('../harness.mjs');
const { default: Counter } = await import('./Counter.svelte');
const { default: V2 } = await import('./CounterV2.svelte');
await report(R, 'HMR', Counter, async (c) => {
	await c.step('click x2', () => { R.emit(c.find('button'), 'click', {}); c.flushSync(); R.emit(c.find('button'), 'click', {}); });
	await c.step('hot swap Counter -> CounterV2', () => accepts['Counter.svelte']({ default: V2 }));
	await c.step('click after swap', () => R.emit(c.find('button'), 'click', {}));
});
