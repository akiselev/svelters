// Handles are never reused and the dead-handle memory is bounded.
const h = globalThis.__svelters_host;
const N = 5000;
const seen = new Set();
let first = 0;
let last = 0;
for (let i = 0; i < N; i++) {
  const x = h.create('label');
  if (seen.has(x)) throw new Error(`handle #${x} was issued twice`);
  if (!Number.isSafeInteger(x)) throw new Error(`handle ${x} is not exact in JS`);
  seen.add(x);
  if (i === 0) first = x;
  last = x;
  h.destroy(x);
}
const msg = (x) => {
  try {
    h.getProp(x, 'label');
    return 'ok';
  } catch (e) {
    return e.message;
  }
};
console.log(`first: ${msg(first)}`);
console.log(`last: ${msg(last)}`);
