<script>
  // EM2: an animation-frame loop started from an event handler (not an effect): per-root rAF tagging
  // of dispatches, so the loop stops while the root is hidden and resumes when it is shown.
  import { root } from 'svelters:embed';

  const r = root();
  let ticks = $state(0);
  let running = false;
  function tick() {
    ticks++;
    requestAnimationFrame(tick);
  }
  $effect(() => {
    r.title = `ticks ${ticks}`;
  });
</script>

<box gap={4}>
  <button
    onclicked={() => {
      if (!running) {
        running = true;
        requestAnimationFrame(tick);
      }
    }}>Animate</button
  >
  <label>ticks {ticks}</label>
</box>
