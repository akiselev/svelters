// Manual benchmark: time to reverse N native rows through the renderer API on the TestHost.
// Usage: node --conditions=custom-renderer test/reorder-bench.mjs [N ...]
import { createTestHost } from '@svelters/testhost';

const th = createTestHost();
globalThis.__svelters_host = th.host;
const { default: r, root } = await import('../src/index.js');
const tick = () => new Promise((res) => setImmediate(res));

for (const n of process.argv.slice(2).map(Number).concat(process.argv.length > 2 ? [] : [1000, 10000])) {
  const win = r.createElement('window');
  const box = r.createElement('box');
  r.insert(win, box, null);
  r.insert(root, win, null);
  const rows = [];
  for (let i = 0; i < n; i++) {
    const row = r.createElement('label');
    r.insert(row, r.createTextNode(`row ${i}`), null);
    r.insert(box, row, null);
    rows.push(row);
  }
  await tick();
  th.lines.length = 0;
  const t0 = performance.now();
  for (const row of [...rows].reverse()) r.insert(box, row, box.first === row ? null : box.first);
  await tick();
  console.log(`reverse ${n} rows: ${(performance.now() - t0).toFixed(1)} ms`);
  th.lines.length = 0;
  r.remove(win);
  await tick();
  th.lines.length = 0;
}
