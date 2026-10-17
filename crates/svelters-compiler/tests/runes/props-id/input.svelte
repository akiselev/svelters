<script>
  const uid = $props.id();
  let { a, b = 1, c = compute(), d = [1], e = $bindable({}), f = $bindable(), 'g-h': g, ...rest } = $props();
  function compute() {
    return 2;
  }
  function touch() {
    b = 2;
    e.x = 1;
  }
</script>

<label label={`${uid} ${a} ${b} ${c} ${d} ${e} ${f} ${g} ${rest.z}`} />
