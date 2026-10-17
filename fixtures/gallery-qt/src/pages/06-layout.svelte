<!-- kinds: QHBoxLayout QVBoxLayout QGridLayout QFormLayout QStackedLayout QBoxLayout QMdiArea QMdiSubWindow QDockWidget QMainWindow QToolBar -->
<script>
  // Containers B: mdiarea and dockwidget. Layouts are reached through `box` and `frame` (orientation).
  // The mdiarea `subwindows` slot (append-only, nested) and the dockwidget `widget` slot are verified.
  // `grid` cells use parent:column/row/columnSpan/rowSpan; `box` children use parent:stretch/align.
  let events = $state(0);
  let areasOpen = $state(true);
</script>

<box orientation="vertical" spacing="4">
  <box orientation="horizontal" spacing="6">
    <label label="box horizontal: a QBoxLayout" parent:stretch={2} parent:align="left" />
    <label label="second child" parent:stretch={1} parent:align="right" />
  </box>
  <grid>
    <label label="grid 0,0" parent:column={0} parent:row={0} />
    <label label="grid 1,0" parent:column={1} parent:row={0} />
    <label label="grid spans two columns" parent:column={0} parent:row={1} parent:columnSpan={2} />
  </grid>
  <mdiarea viewMode="sub-window-view" activationOrder="creation-order" tabsClosable={true} tabsMovable={true}
           tabPosition="north" tabShape="rounded" documentMode={false} minimumHeight={80}>
    <label label="sub-window one" />
    <label label="sub-window two" />
  </mdiarea>
  <dockwidget windowTitle="dockwidget" floating={true} minimumHeight={40}
              ontop-level-changed={() => events++} onvisibility-changed={() => events++}
              onfeatures-changed={() => events++} onallowed-areas-changed={() => events++}>
    <label label="dock widget content" />
  </dockwidget>
  <label label="dock events: {events}" />
  <button label="main window areas" onclick={() => (areasOpen = !areasOpen)} />
  <!-- A second window: toolbars and dock widgets are children of the window, placed by `parent:area`
       (top/left/right/bottom) and `parent:breakBefore`; the label is the central widget. -->
  {#if areasOpen}
    <window title="Main window areas" show="modeless">
      <toolbar parent:area="top" movable={false} />
      <toolbar parent:area="top" parent:breakBefore={true} movable={false} />
      <toolbar parent:area="left" movable={false} />
      <dockwidget parent:area="right" windowTitle="Inspector">
        <label label="dock widget on the right" />
      </dockwidget>
      <label label="central widget" />
    </window>
  {/if}
</box>
