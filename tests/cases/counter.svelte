<!--
@case A click reaches JS and JS updates the label (fixtures/counter, counter-qt, counter-egui).
-->
<script>
  import { Window, Column, Button } from 'svelters:testkit';

  let count = $state(0);
</script>

<Window title="Counter">
  <Column>
    <label>Count: {count}</label>
    <Button text="Increment" onclick={() => count++} />
  </Column>
</Window>

<!--
@test
expect "Count: 0"
click "Increment"
expect "Count: 1"
click "Increment"
click "Increment"
expect "Count: 3"
expect no "Count: 1"
snapshot "three"
-->
