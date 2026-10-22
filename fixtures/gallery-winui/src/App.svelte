<script>
  // WinUI 3 widget gallery (WP-W): a sidebar of family pages (src/pages/*.svelte, sorted by file name) and the
  // selected page on the right. Each page starts with `<!-- kinds: ... -->` (tools/metadata/coverage.py reads it;
  // `counter-win --fixture gallery-winui --selftest` clicks every sidebar button and checks those kinds were created).
  // Keep the sidebar the first stack-panel of the window: the selftest finds its buttons there. Handle layout (shared with
  // fixtures/gallery-common/check-pages.mjs): window #1, grid #2, sidebar stack-panel #3, sidebar button i = #(4 + i).
  const modules = import.meta.glob('./pages/*.svelte', { eager: true });
  const pages = Object.keys(modules)
    .sort()
    .map((path) => ({ name: path.slice('./pages/'.length, -'.svelte'.length), component: modules[path].default }));
  const title = (name) => name.replace(/^\d+-/, '').replace(/-/g, ' ');
  let current = $state(0);
  const Page = $derived(pages[current].component);
</script>

<window title="svelters gallery (WinUI)">
  <grid column-definitions="Auto, *" column-spacing="12" padding="12">
    <stack-panel parent:column={0} orientation="vertical" spacing="4">
      {#each pages as p, i}
        <button onclick={() => (current = i)}>{title(p.name)}</button>
      {/each}
    </stack-panel>
    <scroll-viewer parent:column={1}>
      <stack-panel orientation="vertical" spacing="8">
        <text-block font-size={20}>{title(pages[current].name)}</text-block>
        <Page />
      </stack-panel>
    </scroll-viewer>
  </grid>
</window>
