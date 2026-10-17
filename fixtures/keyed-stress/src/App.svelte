<script>
  import { onMount } from 'svelte';

  let rows = $state([1, 2, 3, 4, 5, 6]);
  let next = 7;
  // Filled in once every step ran: the order the rows must have natively.
  let expected = $state('');

  // Each step runs in its own timer task, so each one is a separate keyed-each update.
  const steps = [
    () => (rows = rows.toReversed()),
    () => rows.splice(2, 0, next++),
    () => (rows = [next++, ...rows]),
    () => (rows = [...rows, next++]),
    () => (rows = rows.filter((_, i) => i !== 0 && i !== 4)),
    () => (rows = rows.toSorted((a, b) => ((a * 7) % 11) - ((b * 7) % 11))),
    () => (rows = rows.toReversed()),
    () => (rows = [...rows.slice(3), ...rows.slice(0, 3)]),
    () => (rows = rows.filter((r) => r % 2 === 0)),
    () => (rows = [...rows, 100, 101].reverse()),
  ];

  onMount(() => {
    let i = 0;
    const tick = () => {
      if (i < steps.length) {
        steps[i++]();
        setTimeout(tick, 1);
      } else {
        expected = rows.join(',');
      }
    };
    setTimeout(tick, 1);
  });
</script>

<window title="Keyed stress">
  <box orientation="vertical" spacing="2">
    <label>{expected}</label>
    <box orientation="vertical" spacing="2">
      {#each rows as id (id)}
        <label>{id}</label>
      {/each}
    </box>
  </box>
</window>
