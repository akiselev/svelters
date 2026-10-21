<script>
  // The counter on egui (G1): the egui vocabulary (label, button, checkbox, text-edit, slider in a
  // taffy `box`). `bind:` on the props the widgets change (text, value, checked) lowers onto exact
  // observation; layout props are CSS names and may come from <style> (D3).
  let count = $state(0);
  let name = $state('');
  let level = $state(20);
  let agree = $state(false);
</script>

<window title="Counter" default-width={360} default-height={260}>
  <box class="column">
    <label>Count: {count}</label>
    <button onclicked={() => count++}>Increment</button>
    <box class="row">
      <text-edit hint-text="Name" flex-grow={1} bind:text={name} />
      <label>Hello {name}</label>
    </box>
    <slider min={0} max={100} step={1} accessible-name="Level" bind:value={level} />
    <checkbox bind:checked={agree}>Level {level}, agree: {agree ? 'yes' : 'no'}</checkbox>
  </box>
</window>

<style>
  .column {
    gap: 8px;
  }
  .row {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }
</style>
