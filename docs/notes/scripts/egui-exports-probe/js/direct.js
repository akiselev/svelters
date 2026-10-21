'use strict';
// Part A (direct rquickjs). Globals from direct.rs: Sim, log, now, sharedView, makeBuffer, reclaimSum, scaleAsync, N.
const ITER = 1_000_000;
const ms = (t) => (now() - t).toFixed(3);
const per = (t, n) => (((now() - t) * 1e6) / n).toFixed(1);

// --- call cost -------------------------------------------------------------------------------
{
  let acc = 0;
  function jsAdd(x) { acc += x; return acc; }
  for (let i = 0; i < ITER; i++) jsAdd(1); // warm
  let t = now();
  for (let i = 0; i < ITER; i++) jsAdd(1);
  log(`plain JS function call: ${per(t, ITER)} ns/call`);

  const sim = new Sim(16);
  for (let i = 0; i < ITER; i++) sim.add(1);
  t = now();
  for (let i = 0; i < ITER; i++) sim.add(1);
  log(`rquickjs #[class] method sim.add(1): ${per(t, ITER)} ns/call`);
  t = now();
  let g = 0;
  for (let i = 0; i < ITER; i++) g += sim.gain;
  log(`rquickjs #[class] field getter sim.gain: ${per(t, ITER)} ns/read`);
}

// --- zero-copy view vs copy ------------------------------------------------------------------
{
  const sim = new Sim(N);
  let t = now();
  const v = sim.view();
  log(`view over Arc<[f32]> (${(N * 4) >> 20} MiB): ${ms(t)} ms, v[N-1]=${v[N - 1]}, Arc refs=${sim.refs}`);
  t = now();
  const c = sim.copy();
  log(`copy (${(N * 4) >> 20} MiB): ${ms(t)} ms, c[N-1]=${c[N - 1]}`);
  let threw = 'no';
  try { v[0] = 42; } catch (e) { threw = `${e.name}: ${e.message}`; }
  log(`write to immutable view: threw=${threw}; v[0] still ${v[0]}`);
  t = now();
  let s = 0;
  for (let i = 0; i < v.length; i += 1) s += v[i];
  log(`JS sum over the zero-copy view: ${ms(t)} ms (sum ${s})`);
  t = now();
  s = 0;
  for (let i = 0; i < c.length; i += 1) s += c[i];
  log(`JS sum over the copied array: ${ms(t)} ms (sum ${s})`);
  globalThis.keepShared = sharedView();
}

// --- transfer() of a JS-allocated buffer -------------------------------------------------------
{
  const big = new Float32Array(N).fill(1);
  const t = now();
  const moved = big.buffer.transfer();
  log(`ArrayBuffer.transfer() of ${(N * 4) >> 20} MiB: ${ms(t)} ms; old length ${big.length}, new byteLength ${moved.byteLength}`);
}

// --- Rust-allocated buffer, filled by JS, reclaimed by Rust without a copy ---------------------
{
  const b = makeBuffer(N);
  b.fill(0.5);
  const t = now();
  const sum = reclaimSum(b);
  log(`reclaim Rust Vec from JS-filled buffer + sum: ${ms(t)} ms (sum ${sum}); JS length after: ${b.length}`);
}

// --- JS buffer → rayon worker → back to JS through a Promise ----------------------------------
{
  const a = new Float32Array(N).fill(2);
  const t = now();
  scaleAsync(a, 3).then((out) => {
    log(`scaleAsync round trip: ${ms(t)} ms; out[0]=${out[0]} out.length=${out.length}; input length now ${a.length}`);
  });
  log(`scaleAsync returned to JS after ${ms(t)} ms (call + transfer + spawn; first call starts the rayon pool)`);
  const b = new Float32Array(N).fill(1);
  const t2 = now();
  scaleAsync(b, 5).then((out) => { const t3 = now(); log(`second scaleAsync round trip: ${ms(t2)} ms; out[0]=${out[0]}`); log(`  (log in reaction took ${ms(t3)} ms)`); });
  log(`second scaleAsync returned to JS after ${ms(t2)} ms`);
}
