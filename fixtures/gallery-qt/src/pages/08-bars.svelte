<!-- kinds: QTabBar QDialogButtonBox QStatusBar QMenuBar QToolBar -->
<script>
  // Bars: tabbar, dialogbuttonbox, statusbar, menubar, toolbar. Tabs, menu items and toolbar
  // actions cannot be added from JS yet (no tab/action slots, gap G7), so the bars show their
  // plain props and signals. The window's own menubar/statusbar slots are covered by QMainWindow.
  let tab = $state(0);
  let echoTab = $state(0);
  let tabEvents = $state(0);
  let boxEvents = $state(0);
  let messages = $state(0);
  let toolEvents = $state(0);
  $effect(() => {
    tab = 1;
  });
</script>

<box orientation="vertical" spacing="4">
  <tabbar bind:currentIndex={tab} onchanged={() => echoTab++} shape="rounded-north"
          tabsClosable={true} movable={true} expanding={false} drawBase={true} documentMode={false}
          ontab-close-requested={() => tabEvents++} ontab-bar-clicked={() => tabEvents++}
          ontab-moved={() => tabEvents++} />
  <label label="echo currentIndex: {echoTab}" />
  <label label="tab events: {tabEvents}" />
  <dialogbuttonbox orientation="horizontal" centerButtons={true} standardButtons={["ok", "cancel", "help"]}
                   onaccepted={() => boxEvents++} onrejected={() => boxEvents++}
                   onhelp-requested={() => boxEvents++} />
  <label label="button box events: {boxEvents}" />
  <menubar defaultUp={false} />
  <toolbar movable={false} floatable={false} orientation="horizontal" toolButtonStyle="icon-only"
           onmovable-changed={() => toolEvents++} onorientation-changed={() => toolEvents++} />
  <label label="toolbar events: {toolEvents}" />
  <statusbar sizeGripEnabled={false} onmessageChanged={() => messages++} />
  <label label="status messages: {messages}" />
</box>
