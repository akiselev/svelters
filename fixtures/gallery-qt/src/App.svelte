<script>
  // Qt widget gallery: a sidebar of family pages (src/pages/*.svelte, sorted by file name) and
  // the selected page on the right. Handle layout is fixed so screenshot tests can click a page:
  // main-window #1, outer box-layout #2, sidebar box-layout #3, sidebar push-button i = #(4 + i). Keep the sidebar first.
  // Only annotated Qt props exist.
  const modules = import.meta.glob('./pages/*.svelte', { eager: true });
  const pages = Object.keys(modules)
    .sort()
    .map((path) => ({ name: path.slice('./pages/'.length, -'.svelte'.length), component: modules[path].default }));
  const title = (name) => name.replace(/^\d+-/, '').replace(/-/g, ' ');
  let current = $state(0);
  // The window size: Qt sizes a window to its content, which is too small for the pages.
  let win = $state(null);
  $effect(() => {
    win?.call('resize', 960, 640);
  });
  const Page = $derived(pages[current].component);
</script>

<main-window window-title="svelters gallery (Qt)" bind:this={win}>
  <box-layout direction="left-to-right" spacing="12">
    <box-layout direction="top-to-bottom" spacing="4">
      {#each pages as p, i}
        <push-button text={title(p.name)} onclicked={() => (current = i)} />
      {/each}
    </box-layout>
    <scroll-area>
      <box-layout direction="top-to-bottom" spacing="8">
        <label text={title(pages[current].name)} />
        <Page />
      </box-layout>
    </scroll-area>
  </box-layout>
</main-window>
