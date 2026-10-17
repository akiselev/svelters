<!-- Destructured $state / $derived declarations and destructuring assignments. -->
<script>
  let [left, right] = $state(['L', 'R']);
  let { size, tags = ['none'] } = $state({ size: 1 });
  let { area, perimeter } = $derived({ area: size * size, perimeter: 4 * size });

  function swap() {
    [left, right] = [right, left];
  }

  function grow() {
    ({ size } = { size: size + 1 });
  }
</script>

<window title="Destructuring">
  <box orientation="vertical" spacing="4">
    <label>{left}{right} size={size} area={area} perimeter={perimeter} tags={tags.join(',')}</label>
    <button onclicked={swap}>Swap</button>
    <button onclicked={grow}>Grow</button>
  </box>
</window>
