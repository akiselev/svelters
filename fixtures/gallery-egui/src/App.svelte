<script>
  // egui widget gallery: a sidebar of family pages (src/pages/*.svelte, sorted by file name) and the
  // selected page on the right. crates/svelters-egui/tests/gallery.rs opens every page by its
  // sidebar button (labelled with the page's title) and drives it with kittest.
  const modules = import.meta.glob('./pages/*.svelte', { eager: true });
  const pages = Object.keys(modules)
    .sort()
    .map((path) => ({ name: path.slice('./pages/'.length, -'.svelte'.length), component: modules[path].default }));
  const title = (name) => name.replace(/^\d+-/, '').replace(/-/g, ' ');
  let current = $state(0);
  const Page = $derived(pages[current].component);
</script>

<window title="svelters gallery (egui)" default-width={960} default-height={640}>
  <box flex-direction="row" gap={12} flex-grow={1}>
    <box gap={4}>
      {#each pages as p, i}
        <button onclicked={() => (current = i)}>{title(p.name)}</button>
      {/each}
    </box>
    <box gap={8} flex-grow={1}>
      <label>Page: {title(pages[current].name)}</label>
      <Page />
    </box>
  </box>
</window>
