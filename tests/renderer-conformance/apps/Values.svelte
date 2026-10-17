<script>
  let n = $state(1);
  const obj = { a: 1 };
  function cb() {}
  export function inc() { n++; }
  let rest = $derived({ spreadNum: n, spreadFn: cb });
</script>
<widget num={n} float={1.5} bool={true} falsy={false} nul={null} undef={undefined} obj={obj} arr={[1, 2]} fn={cb} str="s" value={n} checked={n > 1} class="a b" style="color: red" {...rest} />
