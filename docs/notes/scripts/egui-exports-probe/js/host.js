// Part B (svelters host, ABI v3). Runs as the bundle entry module.
const H = globalThis.__svelters_host;
const sim = H.create('sim');
H.listen(sim, 'tick', true);
H.listen(sim, 'start', true);
const ms = (t) => (performance.now() - t).toFixed(3);
const per = (t, n) => (((performance.now() - t) * 1e6) / n).toFixed(1);
let ticks = 0;

async function main() {
  const ITER = 200_000;
  for (let i = 0; i < 1000; i++) H.call(sim, 'add', [1]);
  let t = performance.now();
  for (let i = 0; i < ITER; i++) H.call(sim, 'add', [1]);
  console.log(`host.call sync add(F64)->F64 through the C ABI: ${per(t, ITER)} ns/call`);

  const SEQ = 2000;
  t = performance.now();
  for (let i = 0; i < SEQ; i++) await H.call(sim, 'noop', []);
  console.log(`async noop on rayon, sequential await: ${(((performance.now() - t) * 1000) / SEQ).toFixed(1)} us/round trip`);

  const PAR = 10_000;
  t = performance.now();
  await Promise.all(Array.from({ length: PAR }, () => H.call(sim, 'noop', [])));
  console.log(`async noop on rayon, ${PAR} in flight: ${ms(t)} ms total, ${(((performance.now() - t) * 1000) / PAR).toFixed(2)} us/op`);

  H.call(sim, 'ticks', [true]);
  ticks = 0;
  t = performance.now();
  const r = await H.call(sim, 'step', [200]);
  const took = ms(t);
  H.call(sim, 'ticks', [false]);
  console.log(`await step(200 ms CPU on a worker): ${took} ms; 1 ms tick events delivered to JS meanwhile: ${ticks} (result ${r > 0})`);

  // four 200 ms jobs in parallel
  t = performance.now();
  await Promise.all([1, 2, 3, 4].map(() => H.call(sim, 'step', [200])));
  console.log(`4 x step(200) in parallel: ${ms(t)} ms`);

  const N = 64 << 20;
  t = performance.now();
  const b = H.call(sim, 'bytes', [N]);
  console.log(`SVL_BYTES result of ${N >> 20} MiB through the ABI (Vec + JS copy): ${ms(t)} ms, byteLength ${b.byteLength}`);

  H.call(sim, 'finish', []);
}

globalThis.__svelters_dispatch = (h, signal) => {
  if (signal === 'tick') ticks++;
  else if (signal === 'start') main().catch((e) => { console.log('main failed: ' + e + ' ' + e.stack); H.call(sim, 'finish', []); });
};
