<script lang="ts">
  import { untrack } from 'svelte';
  import { Tally, Notebook, greet, fib, crunch, crunchBlocking, wordCount, type Summary, type Step } from 'svelters:app/demo';
  import { telemetry, setSpeed, burst, burstStore, burstSize, Bursts } from 'svelters:app/telemetry';
  import { persist } from '@svelters/types/core';

  const tally = new Tally(5);
  // E3: a persisted object is the same Rust object after a hot reload (the others die with the runtime).
  const notes = persist('notes', () => new Notebook());
  const first = notes.add('first note');

  let count = $state(tally.count);
  let step: Step = $state(tally.step);
  let error = $state('');
  let summary: Summary = $state(tally.summary(null));

  function bump() {
    try {
      count = tally.bump();
      error = '';
    } catch (e) {
      error = (e as Error).message;
    }
    summary = tally.summary(`after ${summary.bumps + 1} bumps`);
  }

  function toggle() {
    tally.step = step === 'one' ? 'ten' : 'one';
    step = tally.step;
  }

  console.log(`exports: ${greet('Svelte')} fib(30)=${fib(30)} same=${notes.get(0) === first} notes=${notes.len}`);

  // E1: a CPU job on a worker thread. A JS timer keeps ticking while it runs (the JS thread is free);
  // the same job on the JS thread freezes it.
  let progress = $state(0);
  let crunchText = $state('idle');
  let ctl: AbortController | null = $state(null);
  let ticks = 0;

  async function startCrunch() {
    ctl = new AbortController();
    progress = 0;
    const t0 = ticks;
    const timer = setInterval(() => ticks++, 5);
    try {
      const r = await crunch(200, { signal: ctl.signal, onProgress: (p) => (progress = p) });
      crunchText = `crunched ${r.primes} primes in ${r.ms.toFixed(0)} ms, ticks ${ticks - t0}`;
    } catch (e) {
      crunchText = `crunch: ${(e as { code?: string }).code}`;
    } finally {
      clearInterval(timer);
      ctl = null;
    }
  }

  function crunchHere() {
    const t0 = ticks;
    const timer = setInterval(() => ticks++, 5);
    const r = crunchBlocking(200);
    clearInterval(timer);
    crunchText = `blocked: crunched ${r.primes} primes in ${r.ms.toFixed(0)} ms, ticks ${ticks - t0}`;
  }

  // An async fn on the JS thread (it awaits a worker inside).
  let words = $state(0);
  wordCount('the quick brown fox jumps').then((n) => (words = n));

  // E3: `telemetry` is a Rust store fed by a service thread. Its getters are reactive (no $state mirror),
  // and `speed` is writable, so the scale binds to it; `on_write` clamps it to 0.5..=10.
  let speedRuns = $state(0);
  $effect(() => {
    telemetry.speed;
    speedRuns = untrack(() => speedRuns) + 1;
  });

  // E3 measurement: N worker writes to a store land in one change batch per checkpoint; N events are
  // delivered one by one (each one a Svelte flush).
  const N = burstSize();
  let storeTarget = 0;
  let storeT0 = 0;
  let storeRuns = 0;
  let storeText = $state('');
  $effect(() => {
    const v = burst.value;
    if (storeTarget === 0) return;
    storeRuns++;
    if (v === storeTarget) storeText = `store burst: ${N} writes, ${storeRuns} effect run(s), ${(performance.now() - storeT0).toFixed(1)} ms`;
  });
  function burstStores() {
    storeTarget = untrack(() => burst.value) + N;
    storeRuns = 0;
    storeT0 = performance.now();
    burstStore(N);
  }

  const bursts = new Bursts();
  let eventValue = $state(0);
  let eventT0 = 0;
  let eventRuns = 0;
  let eventText = $state('');
  bursts.on('value', (v) => (eventValue = v));
  $effect(() => {
    const v = eventValue;
    if (v === 0) return;
    eventRuns++;
    if (v === N) eventText = `event burst: ${N} events, ${eventRuns} effect run(s), ${(performance.now() - eventT0).toFixed(1)} ms`;
  });
  function burstEvents() {
    eventRuns = 0;
    eventT0 = performance.now();
    bursts.fire(N);
  }
</script>

<window title="Rust exports" default-width={360} default-height={260}>
  <box orientation="vertical" spacing={6}>
    <label>{greet('Svelte')}</label>
    <label>count {count} (step {step})</label>
    <label>bumps {summary.bumps}</label>
    <button onclicked={bump}>Bump</button>
    <button onclicked={toggle}>Toggle step</button>
    {#if error}<label>{error}</label>{/if}
    <label>words {words}</label>
    <progress-bar fraction={progress}></progress-bar>
    <label>{crunchText}</label>
    {#if ctl}
      <button onclicked={() => ctl?.abort()}>Cancel</button>
    {:else}
      <button onclicked={startCrunch}>Crunch</button>
      <button onclicked={crunchHere}>Crunch on the UI thread</button>
    {/if}
    <label>samples {telemetry.samples > 0 ? 'flowing' : 'waiting'}</label>
    <label>speed {telemetry.speed.toFixed(1)}</label>
    <label>speed runs {speedRuns}</label>
    <label>{telemetry.log}</label>
    <scale lower={0} upper={20} step-increment={0.5} page-increment={1} digits={1} draw-value={true} hexpand={true} bind:value={telemetry.speed} />
    <button onclicked={() => setSpeed(4)}>Speed 4 from Rust</button>
    <button onclicked={burstStores}>Burst store</button>
    <button onclicked={burstEvents}>Burst events</button>
    <label>{storeText}</label>
    <label>{eventText}</label>
  </box>
</window>
