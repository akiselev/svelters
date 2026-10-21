<script lang="ts">
  import { Tally, Notebook, greet, fib, type Summary, type Step } from 'svelters:app/demo';

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
</script>

<window title="Rust exports" default-width={360} default-height={200}>
  <box orientation="vertical" spacing={6}>
    <label>{greet('Svelte')}</label>
    <label>count {count} (step {step})</label>
    <label>bumps {summary.bumps}</label>
    <button onclicked={bump}>Bump</button>
    <button onclicked={toggle}>Toggle step</button>
    {#if error}<label>{error}</label>{/if}
  </box>
</window>
