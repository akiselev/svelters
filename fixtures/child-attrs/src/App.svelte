<script>
  let order = $state(['a', 'b', 'c']);
  let wide = $state(false);
</script>

<window title="Attrs">
  <grid>
    {#each order as id, i (id)}
      <label parent:column={i % 2} parent:row={Math.floor(i / 2)} parent:column-span={wide && i === 0 ? 2 : 1}>{id}</label>
    {/each}
    <button
      parent:column={0}
      parent:row={5}
      onclicked={() => {
        order = [...order.slice(1), order[0]];
        wide = !wide;
      }}>Rotate</button
    >
  </grid>
</window>
