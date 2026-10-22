<script>
  // EM1: a root with host props (title, n, items), local state, a keyed list with anchors, and its
  // root handle (title for the host's tab, emit, visible).
  import { root } from 'svelters:embed';

  let { title = 'Panel', n = 0, items = [] } = $props();
  let clicks = $state(0);
  const r = root();
  $effect(() => {
    r.title = `${title} (${clicks})`;
  });
</script>

<box gap={4}>
  <label anchor="heading">{title}: {n}</label>
  <button
    onclicked={() => {
      clicks++;
      r.emit('clicked', { title, clicks });
    }}>Inc {title}</button
  >
  <label>{title} clicks: {clicks}</label>
  <label>{title} visible: {r.visible}</label>
  {#each items as it (it)}
    <label anchor={`item-${it}`}>{title} item {it}</label>
  {/each}
</box>
