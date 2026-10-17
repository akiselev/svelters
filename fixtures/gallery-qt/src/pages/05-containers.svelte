<!-- kinds: QFrame QGroupBox QScrollArea QSplitter QStackedWidget QTabWidget QToolBox -->
<script>
  // Containers A: frame, group-box, scroll-area, splitter, stacked-widget, tab-widget, tool-box.
  // Tab and tool-box pages are `nested` slots (the pages live in an internal widget); tab labels are
  // child attributes (`parent:tab-text`).
  let page = $state(0);
  let echoPage = $state(0);
  let toggles = $state(0);
  let moved = $state(0);
  let tabEvents = $state(0);
  let tab = $state(0);
  let echoTab = $state(0);
  let item = $state(0);
  let echoItem = $state(0);
  let checked = $state(true);
  let echoChecked = $state(0);
  // Inserting the first page emits a native currentChanged(0) while the page mounts; echo counting
  // starts with the app's own write after mount, which must not come back as a change event.
  let armed = false;
  $effect(() => {
    setTimeout(() => { armed = true; page = 1; tab = 1; item = 1; checked = false; }, 0);
  });
</script>

<box-layout direction="top-to-bottom" spacing="4">
  <frame direction="top-to-bottom" frame-shape="box" frame-shadow="sunken" line-width={2}>
    <label text="frame: a QFrame with a vertical layout" />
  </frame>
  <group-box title="group-box" checkable={true} flat={false} bind:checked={checked}
            ontoggled={() => { toggles++; if (armed) echoChecked++; }}>
    <label text="inside the group box (toggled: {toggles})" />
  </group-box>
  <splitter orientation="horizontal" handle-width={6} children-collapsible={false}
            onsplitter-moved={() => moved++}>
    <label text="left pane" />
    <label text="right pane" />
  </splitter>
  <label text="splitter moved: {moved}" />
  <stacked-widget bind:current-index={page} oncurrent-changed={() => { if (armed) echoPage++; }}>
    <label text="stack page 0" />
    <label text="stack page 1" />
  </stacked-widget>
  <label text="echo current-index: {echoPage}" />
  <label text="echo checked: {echoChecked}" />
  <tab-widget tab-position="north" tab-shape="rounded" tabs-closable={true} movable={true}
             document-mode={false} uses-scroll-buttons={true} bind:current-index={tab}
             ontab-close-requested={() => tabEvents++} ontab-bar-clicked={() => tabEvents++}
             oncurrent-changed={() => { tabEvents++; if (armed) echoTab++; }}>
    <label parent:tab-text="First" parent:tab-tool-tip="the first tab" text="tab page 0" />
    <label parent:tab-text="Second" text="tab page 1" />
  </tab-widget>
  <label text="tab events: {tabEvents}" />
  <label text="echo current-index (tabs): {echoTab}" />
  <tool-box bind:current-index={item} oncurrent-changed={() => { tabEvents++; if (armed) echoItem++; }}>
    <label text="tool box item 0" />
    <label text="tool box item 1" />
  </tool-box>
  <label text="echo current-index (tool-box): {echoItem}" />
  <scroll-area horizontal-scroll-bar-policy="always-off" vertical-scroll-bar-policy="as-needed" widget-resizable={true}>
    <label text="scroll-area content" />
  </scroll-area>
</box-layout>
