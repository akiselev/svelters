<script>
  // Egui twin of timers: setTimeout + a promise chain updating a label.
  import { onMount } from 'svelte';

  let status = $state('waiting');

  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

  onMount(() => {
    (async () => {
      await sleep(10);
      status = 'timer 1';
      await Promise.resolve();
      await sleep(5);
      status = 'timer 2';
      await sleep(20);
      status = 'done';
    })();
  });
</script>

<window title="Timers">
  <label>{status}</label>
</window>
