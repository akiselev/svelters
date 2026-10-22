<script>
  let p = $state(Promise.resolve('hi'));
  let label = $state('');
</script>
<window>
  <box>
    <label label={await p} />
    <label>{await p}</label>
    <button onclicked={async () => (label = await p)} label="ok" />
    {#await p then v}
      <label label={v} />
    {/await}
  </box>
</window>
