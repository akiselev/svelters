<script lang="ts">
  // E2: large Float32Arrays cross to Rust and back without copies (svelters:app/dsp).
  import { noise, smooth, waveform, stats, isWaveform } from 'svelters:app/dsp';
  import type { ReadonlyFloat32Array } from '@svelters/types/core';

  const N = 1 << 20;

  // A read-only view of a table Rust keeps: no copy, and writes would be type errors.
  const wave: ReadonlyFloat32Array = waveform();
  const ws = stats(wave);
  const waveText = `wave ${ws.len} samples, max ${ws.max.toFixed(2)}, in place ${isWaveform(wave)}`;

  let dspText = $state('dsp idle');
  let dspProgress = $state(0);

  async function runSmooth() {
    // Rust makes the samples; the worker gets the same allocation back (the input is detached meanwhile).
    const input = noise(N, 7);
    const before = stats(input).max;
    const r = await smooth(input, 8, { onProgress: (p) => (dspProgress = p) });
    const after = stats(r.samples).max;
    dspText = `smoothed ${r.samples.length} samples, zero-copy ${r.zeroCopy}, input length ${input.length}, max ${before.toFixed(2)} -> ${after.toFixed(2)}`;
  }
</script>

<label>{waveText}</label>
<progress-bar fraction={dspProgress}></progress-bar>
<button onclicked={runSmooth}>Smooth</button>
<label>{dspText}</label>
