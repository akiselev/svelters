<!-- kinds: QTabBar QDialogButtonBox QStatusBar QMenuBar QToolBar -->
<script>
  // Bars: tab-bar, dialog-button-box, status-bar, menu-bar, tool-bar. Tabs, menu items and toolbar
  // actions cannot be added from JS yet (no tab/action slots, gap G7), so the bars show their
  // plain props and signals. The window's own menu-bar/status-bar slots are covered by QMainWindow.
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

<box-layout direction="top-to-bottom" spacing="4">
  <tab-bar bind:current-index={tab} oncurrent-changed={() => echoTab++} shape="rounded-north"
          tabs-closable={true} movable={true} expanding={false} draw-base={true} document-mode={false}
          ontab-close-requested={() => tabEvents++} ontab-bar-clicked={() => tabEvents++}
          ontab-moved={() => tabEvents++} />
  <label text="echo current-index: {echoTab}" />
  <label text="tab events: {tabEvents}" />
  <dialog-button-box orientation="horizontal" center-buttons={true} standard-buttons={["ok", "cancel", "help"]}
                   onaccepted={() => boxEvents++} onrejected={() => boxEvents++}
                   onhelp-requested={() => boxEvents++} />
  <label text="button box events: {boxEvents}" />
  <menu-bar default-up={false} />
  <tool-bar movable={false} floatable={false} orientation="horizontal" tool-button-style="icon-only"
           onmovable-changed={() => toolEvents++} onorientation-changed={() => toolEvents++} />
  <label text="tool-bar events: {toolEvents}" />
  <status-bar size-grip-enabled={false} onmessage-changed={() => messages++} />
  <label text="status messages: {messages}" />
</box-layout>
