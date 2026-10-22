<!-- kinds: StackPanel Grid Border ScrollViewer Viewbox Canvas SplitView Expander TextBlock -->
<script>
  let paneOpen = $state(true);
  let expanded = $state(false);
</script>

<stack-panel orientation="vertical" spacing="8">
  <grid row-definitions="Auto, Auto" column-definitions="*, *" row-spacing="4" column-spacing="4">
    <text-block parent:row={0} parent:column={0}>Grid 0,0</text-block>
    <text-block parent:row={0} parent:column={1}>Grid 0,1</text-block>
    <text-block parent:row={1} parent:column={0} parent:column-span={2}>Grid row 1, both columns</text-block>
  </grid>
  <border background="#20808080" border-brush="#808080" border-thickness="1" corner-radius="6" padding="8">
    <text-block>Inside a border</text-block>
  </border>
  <scroll-viewer height={60}>
    <stack-panel orientation="vertical">
      <text-block>A scroll viewer, line 1</text-block>
      <text-block>line 2</text-block>
      <text-block>line 3</text-block>
      <text-block>line 4</text-block>
    </stack-panel>
  </scroll-viewer>
  <viewbox width={120} height={40} stretch="uniform">
    <text-block>Viewbox scales this</text-block>
  </viewbox>
  <canvas width={200} height={60} background="#10808080">
    <text-block parent:left={10} parent:top={8}>left 10, top 8</text-block>
    <text-block parent:left={100} parent:top={32} parent:z-index={1}>left 100, top 32</text-block>
  </canvas>
  <toggle-switch header="Split view pane" is-on={paneOpen} ontoggled={() => (paneOpen = !paneOpen)} />
  <split-view is-pane-open={paneOpen} display-mode="inline" open-pane-length={140} height={80}>
    <stack-panel slot="pane" orientation="vertical"><text-block>Pane</text-block></stack-panel>
    <text-block>Split view content</text-block>
  </split-view>
  <expander header="Expander" is-expanded={expanded} onexpanding={() => (expanded = true)} oncollapsed={() => (expanded = false)}>
    <text-block>Expanded content ({expanded ? 'open' : 'closed'})</text-block>
  </expander>
</stack-panel>
