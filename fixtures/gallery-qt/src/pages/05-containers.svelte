<!-- kinds: QFrame QGroupBox QScrollArea QSplitter QStackedWidget QTabWidget QToolBox -->
<script>
  // Containers A: frame, groupbox, scrolledwindow, splitter, stackedwidget, tabwidget, toolbox.
  // Tab and tool-box pages are `nested` slots (the pages live in an internal widget); tab labels are
  // child attributes (`parent:tabLabel`).
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

<box orientation="vertical" spacing="4">
  <frame orientation="vertical" frameShape="box" frameShadow="sunken" lineWidth={2}>
    <label label="frame: a QFrame with a vertical layout" />
  </frame>
  <groupbox title="groupbox" checkable={true} flat={false} bind:checked={checked}
            ontoggled={() => { toggles++; if (armed) echoChecked++; }}>
    <label label="inside the group box (toggled: {toggles})" />
  </groupbox>
  <splitter orientation="horizontal" handleWidth={6} childrenCollapsible={false}
            onsplitter-moved={() => moved++}>
    <label label="left pane" />
    <label label="right pane" />
  </splitter>
  <label label="splitter moved: {moved}" />
  <stackedwidget bind:currentIndex={page} onchanged={() => { if (armed) echoPage++; }}>
    <label label="stack page 0" />
    <label label="stack page 1" />
  </stackedwidget>
  <label label="echo currentIndex: {echoPage}" />
  <label label="echo checked: {echoChecked}" />
  <tabwidget tabPosition="north" tabShape="rounded" tabsClosable={true} movable={true}
             documentMode={false} usesScrollButtons={true} bind:currentIndex={tab}
             ontab-close-requested={() => tabEvents++} ontab-bar-clicked={() => tabEvents++}
             onchanged={() => { tabEvents++; if (armed) echoTab++; }}>
    <label parent:tabLabel="First" parent:tabToolTip="the first tab" label="tab page 0" />
    <label parent:tabLabel="Second" label="tab page 1" />
  </tabwidget>
  <label label="tab events: {tabEvents}" />
  <label label="echo currentIndex (tabs): {echoTab}" />
  <toolbox bind:currentIndex={item} onchanged={() => { tabEvents++; if (armed) echoItem++; }}>
    <label label="tool box item 0" />
    <label label="tool box item 1" />
  </toolbox>
  <label label="echo currentIndex (toolbox): {echoItem}" />
  <scrolledwindow horizontalScrollBarPolicy="always-off" verticalScrollBarPolicy="as-needed" widgetResizable={true}>
    <label label="scrolledwindow content" />
  </scrolledwindow>
</box>
