<script>
  // 1000-row keyed list for the P5 time-split baseline. Buttons are the first widgets created,
  // so their handles are stable: Fill=#4 Update=#5 Reverse=#6 Swap=#7 Clear=#8.
  const N = 1000;
  let rows = $state([]);

  function fill() {
    rows = Array.from({ length: N }, (_, i) => ({ id: i, text: `Row ${i}` }));
  }
  function update() {
    rows = rows.map((r) => ({ ...r, text: r.text + '!' }));
  }
  function reverse() {
    rows = rows.toReversed();
  }
  function swap() {
    const next = [...rows];
    [next[1], next[next.length - 2]] = [next[next.length - 2], next[1]];
    rows = next;
  }
  function clear() {
    rows = [];
  }
  function remove(id) {
    rows = rows.filter((r) => r.id !== id);
  }
</script>

<window title="Keyed bench">
  <box orientation="vertical" spacing="4">
    <box orientation="horizontal" spacing="4">
      <button onclick={fill}>Fill</button>
      <button onclick={update}>Update</button>
      <button onclick={reverse}>Reverse</button>
      <button onclick={swap}>Swap</button>
      <button onclick={clear}>Clear</button>
    </box>
    <scrolledwindow>
      <box orientation="vertical" spacing="2">
        {#each rows as row (row.id)}
          <box orientation="horizontal" spacing="6">
            <label>{row.text}</label>
            <button onclick={() => remove(row.id)}>Remove</button>
          </box>
        {/each}
      </box>
    </scrolledwindow>
  </box>
</window>
