<!--
@case A keyed each block: rows follow a reverse, an append and a removal at the front
(fixtures/keyed-list, keyed-list-qt, keyed-list-egui).
-->
<script>
  import { Window, Column, Row, Button } from 'svelters:testkit';

  let next = 4;
  let rows = $state([
    { id: 1, text: 'one' },
    { id: 2, text: 'two' },
    { id: 3, text: 'three' },
  ]);
</script>

<Window title="Keyed each">
  <Column>
    <Row>
      <Button text="Add" onclick={() => rows.push({ id: next, text: `row ${next++}` })} />
      <Button text="Reverse" onclick={() => (rows = rows.toReversed())} />
      <Button text="Drop first" onclick={() => (rows = rows.slice(1))} />
    </Row>
    {#each rows as row (row.id)}
      <label>{row.text}</label>
    {/each}
    <label>{rows.length} rows</label>
  </Column>
</Window>

<!--
@test
expect order "one" "two" "three" "3 rows"
click "Reverse"
expect order "three" "two" "one" "3 rows"
click "Add"
expect order "three" "two" "one" "row 4" "4 rows"
click "Drop first"
expect no "three"
expect order "two" "one" "row 4" "3 rows"
-->
