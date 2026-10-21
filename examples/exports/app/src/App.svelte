<script lang="ts">
  import { Tally, Notebook, greet, fib, crunch, crunchBlocking, wordCount, type Summary, type Step } from 'svelters:app/demo';

  const tally = new Tally(5);
  const notes = new Notebook();
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
  </box>
</window>
