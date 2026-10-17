<script>
  let count = $state(0);
  let items = $state([]);
  let d = $derived(count * 2);
  $inspect(count);
  $inspect(count, count * 2).with((type, ...values) => console.info(type, values));
  function f() {
    $inspect.trace();
    return count;
  }
  const g = async () => {
    $inspect.trace('g');
    await 1;
  };
  function reassign() {
    count = 1;
  }
</script>

<label label={`${count} ${d} ${items.length}`} />
