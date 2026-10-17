// getProp of strings on the Qt backend (native -> JS), best of 7 rounds in ns per op, JS loop included.
// Run with SVL_BENCH=1: `SVL_BENCH=1 xvfb-run -a cargo test --release -p counter-qt --test host`.
const h = globalThis.__svelters_host;
const t = h.create('label');
const pLabel = h.propId ? h.propId(h.kindId('label'), 'text') : 'text';
function bench(name, text, n) {
  h.setProp(t, pLabel, text);
  const f = (k) => {
    let last;
    for (let i = 0; i < k; i++) last = h.getProp(t, pLabel);
    return last;
  };
  f(200);
  let best = 1e18;
  for (let r = 0; r < 7; r++) {
    const t0 = performance.now();
    const v = f(n);
    best = Math.min(best, ((performance.now() - t0) * 1e6) / n);
    if (v !== text) throw new Error(name + ': value changed on the way: ' + v.length);
  }
  console.log(name.padEnd(36) + String(Math.round(best)).padStart(8) + ' ns/op');
}
bench('getProp 11 B ASCII', 'Hello World', 20000);
bench('getProp 1 KiB ASCII', 'abcdefgh'.repeat(128), 20000);
bench('getProp 1 KiB Latin-1', 'abcdefgé'.repeat(128), 10000);
bench('getProp 1 KiB wide (BMP)', 'abcdefg世'.repeat(128), 10000);
bench('getProp 1 KiB wide + astral', 'abcdef\u{1F600}'.repeat(128), 10000);
