<script>
  // The counter on WinUI 3: native tags and member names in kebab-case (StackPanel -> stack-panel,
  // TextChanged -> text-changed). Text children set TextBlock.Text / ContentControl.Content.
  let count = $state(0);
  let name = $state('');
  let level = $state(20);
  let agree = $state(false);
</script>

<window title="Counter">
  <stack-panel orientation="vertical" spacing="8" padding="16">
    <text-block>Count: {count}</text-block>
    <button onclick={() => count++}>Increment</button>
    <grid row-definitions="Auto, Auto" column-definitions="*, *" column-spacing="8" row-spacing="8">
      <text-box parent:row={0} parent:column={0} placeholder-text="Name" ontext-changed={(text) => (name = text)} />
      <text-block parent:row={0} parent:column={1}>Hello {name}</text-block>
      <slider parent:row={1} parent:column={0} minimum="0" maximum="100" value={level} onvalue-changed={(v) => (level = Math.round(v))} />
      <check-box parent:row={1} parent:column={1} onchecked={() => (agree = true)} onunchecked={() => (agree = false)}>Level {level}, agree: {agree ? 'yes' : 'no'}</check-box>
    </grid>
  </stack-panel>
</window>
