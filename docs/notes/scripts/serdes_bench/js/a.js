// Part A: the current host path (__svelters_host.setProp etc.) on a dummy backend.
const H = globalThis.__svelters_host;
const h = H.create('widget');
const N = 10000;
function bench(name, fn) {
  let best = 1e9;
  for (let r = 0; r < 9; r++) { const t0 = performance.now(); fn(); const t = performance.now() - t0; if (t < best) best = t; }
  console.log(`A\t${name}\t${(best * 1e6 / N).toFixed(0)}`);
}
const KB = 'x'.repeat(1024);
const KBU = 'é'.repeat(512) + '✓'.repeat(512);
bench('js loop only', () => { let s = 0; for (let i = 0; i < N; i++) s += i; });
bench('setProp int', () => { for (let i = 0; i < N; i++) H.setProp(h, 'spacing', i); });
bench('setProp f64', () => { for (let i = 0; i < N; i++) H.setProp(h, 'opacity', i + 0.5); });
bench('setProp bool', () => { for (let i = 0; i < N; i++) H.setProp(h, 'visible', (i & 1) === 0); });
bench('setProp str 11B ascii', () => { for (let i = 0; i < N; i++) H.setProp(h, 'label', 'hello world'); });
bench('setProp str 1KiB ascii', () => { for (let i = 0; i < N; i++) H.setProp(h, 'label', KB); });
bench('setProp str 1Ki chars non-latin1', () => { for (let i = 0; i < N; i++) H.setProp(h, 'label', KBU); });
bench('setProp enum as string', () => { for (let i = 0; i < N; i++) H.setProp(h, 'orientation', 'vertical'); });
bench('setProp rect as LIST [x,y,w,h]', () => { for (let i = 0; i < N; i++) H.setProp(h, 'geometry', [i, 2, 300, 400]); });
bench('getProp int', () => { for (let i = 0; i < N; i++) H.getProp(h, 'spacing'); });
bench('call 2 args', () => { for (let i = 0; i < N; i++) H.call(h, 'select_region', [i, i + 3]); });
