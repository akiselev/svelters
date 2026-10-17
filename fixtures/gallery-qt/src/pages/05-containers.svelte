<!-- kinds: QFrame QGroupBox QScrollArea QSplitter QStackedWidget QTabWidget QToolBox -->
<script>
  // Containers A: frame, groupbox, scrolledwindow, splitter, stackedwidget, tabwidget, toolbox.
  // Tab and tool-box pages cannot be added yet (the page slots are not verifiable, see
  // docs/notes/a1-qt-containers-a.md), so those two show props and signals only.
  let page = $state(0);
  let echoPage = $state(0);
  let toggles = $state(0);
  let moved = $state(0);
  let tabEvents = $state(0);
  $effect(() => {
    // a programmatic write after mount: must not come back as a change event
    // pages are inserted after the first write, and inserting the first page emits currentChanged(0)
    setTimeout(() => { page = 1; }, 0);
  });
</script>

<box orientation="vertical" spacing="4">
  <frame orientation="vertical" frameShape="box" frameShadow="sunken" lineWidth={2}>
    <label label="frame: a QFrame with a vertical layout" />
  </frame>
  <groupbox title="groupbox" checkable={true} flat={false} ontoggled={() => toggles++}>
    <label label="inside the group box (toggled: {toggles})" />
  </groupbox>
  <splitter orientation="horizontal" handleWidth={6} childrenCollapsible={false}
            onsplitter-moved={() => moved++}>
    <label label="left pane" />
    <label label="right pane" />
  </splitter>
  <label label="splitter moved: {moved}" />
  <stackedwidget bind:currentIndex={page} onchanged={(i) => { if (i !== page) echoPage++; }}>
    <label label="stack page 0" />
    <label label="stack page 1" />
  </stackedwidget>
  <label label="echo currentIndex: {echoPage}" />
  <tabwidget tabPosition="north" tabShape="rounded" tabsClosable={true} movable={true}
             documentMode={false} usesScrollButtons={true}
             ontab-close-requested={() => tabEvents++} ontab-bar-clicked={() => tabEvents++}
             onchanged={() => tabEvents++} />
  <label label="tab events: {tabEvents}" />
  <toolbox onchanged={() => tabEvents++} />
  <scrolledwindow horizontalScrollBarPolicy="always-off" verticalScrollBarPolicy="as-needed" widgetResizable={true}>
    <label label="scrolledwindow content" />
  </scrolledwindow>
</box>
