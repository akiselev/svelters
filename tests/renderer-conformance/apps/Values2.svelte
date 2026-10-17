<script>
  let n = $state(1);
  const obj = { a: 1 };
  function cb() {}
  export function inc() { n++; }
</script>
<widget num={n} obj={obj} fn={cb} model={{ rows: n }} class={{ active: n > 1, big: true }} label="static" />
