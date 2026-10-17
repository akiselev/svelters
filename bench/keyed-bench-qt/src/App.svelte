<script>
  // Qt twin of bench/keyed-bench (Qt vocabulary). 1000-row keyed list for the P5 time-split
  // baseline. Buttons are the first widgets created, so their handles are stable: Fill=#4 Update=#5
  // Reverse=#6 Swap=#7 Clear=#8.
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

<main-window window-title="Keyed bench">
  <box-layout direction="top-to-bottom" spacing="4">
    <box-layout direction="left-to-right" spacing="4">
      <push-button onclicked={fill}>Fill</push-button>
      <push-button onclicked={update}>Update</push-button>
      <push-button onclicked={reverse}>Reverse</push-button>
      <push-button onclicked={swap}>Swap</push-button>
      <push-button onclicked={clear}>Clear</push-button>
    </box-layout>
    <scroll-area>
      <box-layout direction="top-to-bottom" spacing="2">
        {#each rows as row (row.id)}
          <box-layout direction="left-to-right" spacing="6">
            <label>{row.text}</label>
            <push-button onclicked={() => remove(row.id)}>Remove</push-button>
          </box-layout>
        {/each}
      </box-layout>
    </scroll-area>
  </box-layout>
</main-window>
