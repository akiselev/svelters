<!-- Qt-vocabulary copy of ../App.svelte (the qt compile target). -->
<!-- User bindings named like the compiler's internal element constants must not collide. -->
<script>
  let n1 = $state(1);
  let n2 = 'x';
</script>

<main-window window-title="Names {n2}">
  <push-button onclicked={() => n1++}>n1 = {n1}</push-button>
</main-window>
