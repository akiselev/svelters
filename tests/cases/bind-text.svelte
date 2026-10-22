<!--
@case bind:value through a kit text input: native typing reaches JS state, JS writes reach the input
(fixtures/bind-value, bind-value-qt, the bind:text part of counter-egui).
-->
<script>
  import { Window, Column, Button, TextInput } from 'svelters:testkit';

  let name = $state('');
</script>

<Window title="bind">
  <Column>
    <TextInput placeholder="Name" bind:value={name} />
    <label>Hello {name || 'nobody'}</label>
    <Button text="Set Bob" onclick={() => (name = 'Bob')} />
  </Column>
</Window>

<!--
@test
expect "Hello nobody"
type "Name" "Ann"
expect "Hello Ann"
click "Set Bob"
expect "Hello Bob"
# the input shows what JS wrote
expect "Bob"
type "Name" "Zed"
expect "Hello Zed"
expect no "Hello Bob"
-->
