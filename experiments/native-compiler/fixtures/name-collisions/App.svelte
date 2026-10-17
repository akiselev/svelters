<!-- User bindings named like the compiler's internal element constants must not collide. -->
<script>
  let n1 = $state(1);
  let n2 = 'x';
</script>

<window title="Names {n2}">
  <button onclicked={() => n1++}>n1 = {n1}</button>
</window>
