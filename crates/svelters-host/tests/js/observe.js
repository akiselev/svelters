// ABI v2.6 on the log backend: host.observe (refcounted per handle and prop) and the
// __svelters_changed([h, prop, value, ...]) batch of scripted native changes (compared with the last value).
const h = globalThis.__svelters_host;
const win = h.create('window');
const lbl = h.create('label');
const btn = h.create('button');
h.insert(win, lbl, null);

let seen = 0;
globalThis.__svelters_dispatch = () => {};
globalThis.__svelters_changed = (batch) => {
  seen++;
  const [handle, prop, value] = batch;
  h.setProp(lbl, 'label', `${seen}: #${handle === btn ? 'btn' : handle} ${prop} = ${JSON.stringify(value)} (live ${JSON.stringify(h.getProp(handle, prop))})`);
  // The first notification drops one of the two observations (still observed), the second the last.
  if (seen <= 2) console.log(`off -> ${h.observe(btn, 'label', false)}`);
};

console.log(`on -> ${h.observe(btn, 'label', true)}`);
console.log(`nested on -> ${h.observe(btn, 'label', true)}`);
console.log(`off unobserved -> ${h.observe(lbl, 'label', false)}`);
h.setProp(btn, 'label', 'x');

const tmp = h.create('label');
h.observe(tmp, 'label', true);
h.destroy(tmp);
console.log(`off dead -> ${h.observe(tmp, 'label', false)}`);
try {
  h.observe(tmp, 'label', true);
} catch (e) {
  console.log(`on dead -> ${e.message}`);
}
