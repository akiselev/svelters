<script>
  let a = $state(1);
  let b = $state(2);
  let plain1 = 0, plain2 = 0;
  let { left = 0, right } = $props();
  let arr = $state([]);

  function swap() {
    [a, b] = [b, a];
  }
  function swapExpr() {
    return ([a, b] = [b, a]);
  }
  function fromState() {
    [a, b] = arr;
  }
  function objects() {
    ({ a, b = 3 } = { a: 4 });
    ({ left, right } = { left: 1, right: 2 });
  }
  function untouched() {
    [plain1, plain2] = [plain2, plain1];
  }
</script>

<label label={`${a} ${b} ${left} ${right}`} />
