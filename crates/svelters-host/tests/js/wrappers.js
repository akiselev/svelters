// W-J1: node wrappers whose handler captures the wrapper itself. The host holds a wrapper while its
// node lives; after `destroy` only the cycle (wrapper -> handler -> wrapper) is left, which the GC
// collects because the wrapper's gc_mark reports its handlers (tests/fast_ops.rs).
const h = globalThis.__svelters_host;
const win = h.create('window');
const kept = h.create('label');
const dropped = h.create('label');
h.insert(win, kept, null);
h.insert(win, dropped, null);
{
  const a = h.wrap(kept);
  h.wrapHandler(a, 0, () => a);
  const b = h.wrap(dropped);
  h.wrapHandler(b, 0, () => b);
  h.wrapHandler(b, 2, () => h.handleOf(b));
  console.log(`same wrapper ${h.wrap(dropped) === b}`);
  console.log(`handle ${h.handleOf(b) === dropped}`);
  h.destroy(dropped);
  console.log(`after destroy ${h.handleOf(b)}`);
  try {
    h.wrap(dropped);
  } catch (e) {
    console.log(`wrap dead: ${e.message}`);
  }
}
