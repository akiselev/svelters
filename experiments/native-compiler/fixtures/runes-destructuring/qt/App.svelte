<!-- Qt-vocabulary copy of ../App.svelte (the qt compile target). -->
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

<main-window window-title="Destructuring">
  <box-layout direction="top-to-bottom" spacing="4">
    <label>{left}{right} size={size} area={area} perimeter={perimeter} tags={tags.join(',')}</label>
    <push-button onclicked={swap}>Swap</push-button>
    <push-button onclicked={grow}>Grow</push-button>
  </box-layout>
</main-window>
