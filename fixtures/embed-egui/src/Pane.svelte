<script>
  // EM2: a dock pane for the viewport tests (DOCK-W2): a counter whose value reaches the host as the
  // root's title, so a test sees a change without querying a window.
  import { root } from 'svelters:embed';

  let { name = 'pane' } = $props();
  let n = $state(0);
  const r = root();
  $effect(() => {
    r.title = `${name}: ${n}`;
  });
</script>

<box gap={4}>
  <label>{name} count {n}</label>
  <button anchor="inc" onclicked={() => n++}>Inc {name}</button>
</box>
