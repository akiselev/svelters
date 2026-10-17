<!-- kinds: QHBoxLayout QVBoxLayout QGridLayout QFormLayout QStackedLayout QBoxLayout QMdiArea QMdiSubWindow QDockWidget -->
<script>
  // Containers B: mdiarea and dockwidget. Layouts are reached through `box` and `frame` (orientation).
  // Sub-windows cannot be added yet (gap G13) and the dock widget's content slot is not verifiable, so
  // both elements show props and signals only.
  let events = $state(0);
</script>

<box orientation="vertical" spacing="4">
  <box orientation="horizontal" spacing="6">
    <label label="box horizontal: a QBoxLayout" />
    <label label="second child" />
  </box>
  <mdiarea viewMode="sub-window-view" activationOrder="creation-order" tabsClosable={true} tabsMovable={true}
           tabPosition="north" tabShape="rounded" documentMode={false} minimumHeight={80} />
  <dockwidget windowTitle="dockwidget" floating={true} minimumHeight={40}
              ontop-level-changed={() => events++} onvisibility-changed={() => events++}
              onfeatures-changed={() => events++} onallowed-areas-changed={() => events++} />
  <label label="dock events: {events}" />
</box>
