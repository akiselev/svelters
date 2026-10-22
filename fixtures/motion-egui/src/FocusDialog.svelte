<script>
  // EV3 (crates/svelters-egui/tests/focus_keys.rs, loaded from source only like Flip.svelte): a
  // `dialog` focus scope with an `autofocus` field, a default and a cancel button.
  let open = $state(false);
  let name = $state('');
  let result = $state('none');
</script>

<window title="Focus" default-width={360} default-height={260}>
  <box>
    <button onclicked={() => (open = true)}>Open</button>
    {#if open}
      <box focus-scope="dialog">
        <text-edit bind:text={name} accessible-name="name" autofocus />
        <button
          default
          onclicked={() => {
            result = 'ok:' + name;
            open = false;
          }}>OK</button
        >
        <button
          cancel
          onclicked={() => {
            result = 'cancel';
            open = false;
          }}>Cancel</button
        >
      </box>
    {/if}
    <label>Result: {result}</label>
  </box>
</window>
