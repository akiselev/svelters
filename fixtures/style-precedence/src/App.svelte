<script>
  // Setter-rule precedence (ST1, fixed in FX1): explicit attribute > style: / style= > rules >
  // native value. Compared between the stock renderer and the compiled path
  // (experiments/native-compiler/compiled).
  let gap = $state(undefined);
  let name = $state('a');
  let inline = $state('spacing: 3px');
  let on = true;
  function next() {
    on = !on;
    gap = on ? undefined : 4;
    name = on ? 'a' : 'b';
    inline = on ? 'spacing: 3px' : '';
  }
</script>

<window title="Precedence">
  <box orientation="vertical">
    <!-- a nullish style: value falls back to the rule (8), a set one wins (4) -->
    <box class="row" style:spacing={gap}></box>
    <!-- a dynamic id matches #a (5, above .row) while it is "a"; else .row applies (8). Only rules here:
         the log backend reads an unset int prop back as null, which it cannot restore -->
    <box class="row" id={name}></box>
    <!-- the explicit attribute wins over the dynamic inline style -->
    <box spacing={1} style={inline}></box>
    <!-- the dynamic inline style wins over the rule; without it the rule applies again -->
    <box class="row" style={inline}></box>
    <button onclicked={next}>Next</button>
  </box>
</window>

<style>
  .row {
    spacing: 8px;
  }

  #a {
    spacing: 5px;
  }
</style>
