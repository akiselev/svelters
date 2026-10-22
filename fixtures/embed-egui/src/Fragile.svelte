<script>
  // EM1: renders, then throws from a template effect after a click (a render error after mount,
  // caught by the root's boundary); "Throw" is a handler exception (reported, faults after 5).
  let broken = $state(false);
  function explode() {
    throw new Error('Fragile: template exploded');
  }
</script>

<box>
  <label>Fragile ok</label>
  <button onclicked={() => (broken = true)}>Break</button>
  <button
    onclicked={() => {
      throw new Error('handler threw');
    }}>Throw</button
  >
  {#if broken}
    <label>{explode()}</label>
  {/if}
</box>
