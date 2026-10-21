<script>
  // AppKit widget gallery: a sidebar of family pages (src/pages/*.svelte, sorted by file name) and
  // the selected page on the right. Handle layout is fixed so the macOS test can click a page:
  // window #1, outer stack-view #2, sidebar stack-view #3, sidebar button i = #(4 + i). Keep the sidebar first.
  // `cargo run -p counter-mac -- --fixture gallery-appkit`; tested by examples/counter-mac/tests/gallery.rs.
  const modules = import.meta.glob('./pages/*.svelte', { eager: true });
  const pages = Object.keys(modules)
    .sort()
    .map((path) => ({ name: path.slice('./pages/'.length, -'.svelte'.length), component: modules[path].default }));
  const title = (name) => name.replace(/^\d+-/, '').replace(/-/g, ' ');
  let current = $state(0);
  const Page = $derived(pages[current].component);
</script>

<window title="svelters gallery (AppKit)" content-size={{ width: 900, height: 620 }}>
  <stack-view orientation="horizontal" alignment="top" spacing="16" edge-insets={{ top: 16, right: 16, bottom: 16, left: 16 }}>
    <stack-view orientation="vertical" alignment="leading" spacing="4">
      {#each pages as p, i}
        <button title={title(p.name)} bezel-style="push" onaction={() => (current = i)} />
      {/each}
    </stack-view>
    <stack-view orientation="vertical" alignment="leading" spacing="8">
      <text-field string-value={title(pages[current].name)} editable={false} bezeled={false} draws-background={false} />
      <Page />
    </stack-view>
  </stack-view>
</window>
