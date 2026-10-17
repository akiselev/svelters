const h = globalThis.__svelters_host;
const t = h.create('thing');
const check = (c, m) => {
  if (!c) throw new Error(m);
};
// Each read of `icon` mints a boxed handle: 1001, 1002, 1003.
const a = h.getProp(t, 'icon');
check(typeof a.dispose === 'function' && a.disposed === false && a.typeName === 'Icon', 'wrapper');
a.dispose();
check(a.disposed === true, 'disposed');
console.log('disposed');
a.dispose();
console.log('double-dispose-ok');
try {
  h.setProp(t, 'icon', a);
  console.log('accepted a disposed wrapper');
} catch (e) {
  console.log(/live Icon/.test(e.message) ? 'stale-rejected' : e.message);
}
globalThis.keep = h.getProp(t, 'icon'); // 1002 stays alive
let dropped = h.getProp(t, 'icon'); // 1003: dropped without dispose()
dropped = null;
// A live wrapper is accepted back into the backend.
h.setProp(t, 'icon', globalThis.keep);
