<script>
  let never = $state(0);
  let obj = $state({ n: 1 });
  let arr = $state.raw([1, 2]);
  let changed = $state(0);
  let rawChanged = $state.raw({ big: 1 });
  let empty = $state();

  function go() {
    obj.n++;
    changed = { fresh: true };
    changed ||= 5;
    changed *= 2 + 1;
    changed = (1, 2);
    rawChanged = { big: 2 };
    empty = [];
  }
</script>

<label label={`${never} ${obj.n} ${arr.length} ${changed} ${rawChanged.big} ${empty}`} />
