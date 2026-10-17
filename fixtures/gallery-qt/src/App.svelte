<script>
  // Qt widget gallery: a sidebar of family pages (src/pages/*.svelte, sorted by file name) and
  // the selected page on the right. Handle layout is fixed so screenshot tests can click a page:
  // window #1, outer box #2, sidebar box #3, sidebar button i = #(4 + i). Keep the sidebar first.
  // Only annotated Qt props exist; the window size comes from the content.
  const modules = import.meta.glob('./pages/*.svelte', { eager: true });
  const pages = Object.keys(modules)
    .sort()
    .map((path) => ({ name: path.slice('./pages/'.length, -'.svelte'.length), component: modules[path].default }));
  const title = (name) => name.replace(/^\d+-/, '').replace(/-/g, ' ');
  let current = $state(0);
  const Page = $derived(pages[current].component);
</script>

<window title="svelters gallery (Qt)">
  <box orientation="horizontal" spacing="12">
    <box orientation="vertical" spacing="4">
      {#each pages as p, i}
        <button label={title(p.name)} onclick={() => (current = i)} />
      {/each}
    </box>
    <scrolledwindow>
      <box orientation="vertical" spacing="8">
        <label label={title(pages[current].name)} />
        <Page />
      </box>
    </scrolledwindow>
  </box>
</window>
