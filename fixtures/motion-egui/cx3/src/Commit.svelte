<script>
  // CX3: `bind:text|commit` updates `$state` on Enter or on blur after an edit, never per keystroke.
  // The second field is in a focus scope with a default button: Enter goes to the button, and the
  // field still commits first, so the handler reads the committed value.
  let name = $state('Ann');
  let commits = $state(0);
  let title = $state('T');
  let result = $state('none');
  let amount = $state(1);
</script>

<window title="Commit" default-width={400} default-height={300}>
  <box>
    <text-edit bind:text|commit={name} accessible-name="name" oncommit={() => commits++} />
    <label>Name: {name} ({commits})</label>
    <box focus-scope="contain">
      <text-edit bind:text|commit={title} accessible-name="title" />
      <button default onclicked={() => (result = 'ok:' + title)}>OK</button>
    </box>
    <label>Result: {result}</label>
    <drag-value bind:value|commit={amount} accessible-name="amount" />
    <label>Amount: {amount}</label>
  </box>
</window>
