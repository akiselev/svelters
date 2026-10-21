<!-- kinds: NSTabView NSTabViewItem NSSplitView NSBox NSVisualEffectView NSView -->
<script>
  // Containers. AppKit has no layout sizing props yet: the tab and split views take their size from the
  // stack view, and plain views keep the frame they are given.
  let tabs;
  let selections = $state(0);
  let asked = $state(0);
  let locked = $state(false);
</script>

<stack-view orientation="vertical" alignment="leading" spacing="8">
  <tab-view
    bind:this={tabs}
    tab-view-type="top-tabs-bezel-border"
    ontab-view-did-select-tab-view-item={() => selections++}
    ontab-view-should-select-tab-view-item={(e) => {
      asked++;
      if (locked) e.preventDefault();
    }}
  >
    <tab-view-item label="General" identifier={'general'}>
      <stack-view orientation="vertical" alignment="leading" spacing="6">
        <text-field string-value="The first page" editable={false} bezeled={false} draws-background={false} />
        <button title="Next page" onaction={() => tabs.call('select-next-tab-view-item')} />
      </stack-view>
    </tab-view-item>
    <tab-view-item label="Advanced" identifier={'advanced'}>
      <text-field string-value="The second page" editable={false} bezeled={false} draws-background={false} />
    </tab-view-item>
  </tab-view>
  <stack-view orientation="horizontal" spacing="8">
    <button title="Lock the tabs" button-type="switch" onaction={() => (locked = !locked)} />
    <text-field string-value={`selections ${selections}, asked ${asked}`} editable={false} bezeled={false} draws-background={false} />
  </stack-view>
  <split-view vertical={true} divider-style="thin">
    <box title="Left pane">
      <text-field string-value="left" editable={false} bezeled={false} draws-background={false} />
    </box>
    <box title="Right pane">
      <text-field string-value="right" editable={false} bezeled={false} draws-background={false} />
    </box>
  </split-view>
  <stack-view orientation="horizontal" spacing="8">
    <box box-type="custom" fill-color={null} corner-radius={6} border-width={1} title="Custom box">
      <text-field string-value="custom" editable={false} bezeled={false} draws-background={false} />
    </box>
    <box box-type="separator" />
    <visual-effect-view material="sidebar" blending-mode="within-window" state="active">
      <text-field string-value="vibrant" editable={false} bezeled={false} draws-background={false} frame={{ x: 8, y: 8, width: 100, height: 20 }} />
    </visual-effect-view>
    <view tool-tip="a plain view" frame={{ x: 0, y: 0, width: 60, height: 40 }} wants-layer={true} />
  </stack-view>
</stack-view>
