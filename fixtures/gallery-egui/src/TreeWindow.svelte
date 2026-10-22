<script>
  // The `Tree` page in a window of its own: the entry crates/svelters-egui/tests/treeview.rs loads
  // from source (the gallery's App.svelte uses `import.meta.glob`, which only vite has).
  import Tree from './pages/Tree.svelte';
</script>

<window title="tree-view" default-width={520} default-height={480}>
  <Tree />
</window>
