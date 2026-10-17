<script>
  let { a, b: renamed = 5, ...others } = $state({ a: 1, b: 2, c: 3 });
  let [x, , y = 10, ...tail] = $state([1, 2]);
  let { p, q } = $state.raw({ p: 1, q: 2 });
  let source = $state({ s: 1 });
  let { m, n: [nA, nB] } = $derived({ m: a, n: [x, y] });
  let [first, second] = $derived.by(() => [renamed, others]);
  let { s } = $derived(source);

  function swap() {
    a = 10;
    [x, y] = [y, x];
    ({ p, q } = { p: q, q: p });
    p = 3;
    source = { s: 2 };
  }
</script>

<label label={`${a} ${renamed} ${others.c} ${x} ${y} ${tail} ${p} ${q} ${m} ${nA} ${nB} ${first} ${second} ${s}`} />
