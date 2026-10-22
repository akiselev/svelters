<script>
  // egui widget gallery: a sidebar of every page in src/pages/ (sorted by file name; a page's title is
  // its file name without the number prefix) and the selected page on the right. The WIDGETS lanes add
  // pages by adding files: WG2b 20-22, WG4b 40-42, WG4a's identifier-named pages (also embed exports,
  // vite.embed.config.js). crates/svelters-egui/tests/{gallery,wg4b}.rs open pages by their sidebar
  // button and drive them with kittest; tests/gallery_views.rs mounts the embed exports.
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
