<!-- kinds: QHBoxLayout QVBoxLayout QGridLayout QFormLayout QStackedLayout QBoxLayout QMdiArea QMdiSubWindow QDockWidget QMainWindow QToolBar -->
<script>
  // Containers B: mdi-area and dock-widget. Layouts are reached through `box-layout`, `grid-layout` and
  // `frame` (direction).
  // The mdi-area `subwindows` slot (append-only, nested) and the dock-widget `widget` slot are verified.
  // `grid-layout` cells use parent:column/row/column-span/row-span; `box-layout` children use
  // parent:stretch/alignment.
  let events = $state(0);
  let areasOpen = $state(true);
</script>

<box-layout direction="top-to-bottom" spacing="4">
  <box-layout direction="left-to-right" spacing="6">
    <label text="box-layout left-to-right: a QBoxLayout" parent:stretch={2} parent:alignment="left" />
    <label text="second child" parent:stretch={1} parent:alignment="right" />
  </box-layout>
  <grid-layout>
    <label text="grid-layout 0,0" parent:column={0} parent:row={0} />
    <label text="grid-layout 1,0" parent:column={1} parent:row={0} />
    <label text="grid spans two columns" parent:column={0} parent:row={1} parent:column-span={2} />
  </grid-layout>
  <mdi-area view-mode="sub-window-view" activation-order="creation-order" tabs-closable={true} tabs-movable={true}
           tab-position="north" tab-shape="rounded" document-mode={false} minimum-height={80}>
    <label text="sub-window one" />
    <label text="sub-window two" />
  </mdi-area>
  <dock-widget window-title="dock-widget" floating={true} minimum-height={40}
              ontop-level-changed={() => events++} onvisibility-changed={() => events++}
              onfeatures-changed={() => events++} onallowed-areas-changed={() => events++}>
    <label text="dock widget content" />
  </dock-widget>
  <label text="dock events: {events}" />
  <push-button text="main window areas" onclicked={() => (areasOpen = !areasOpen)} />
  <!-- A second window: tool bars and dock widgets are children of the window, placed by `parent:area`
       (top/left/right/bottom) and `parent:tool-bar-break`; the label is the central widget. -->
  {#if areasOpen}
    <main-window window-title="Main window areas" show="modeless">
      <tool-bar parent:area="top" movable={false} />
      <tool-bar parent:area="top" parent:tool-bar-break={true} movable={false} />
      <tool-bar parent:area="left" movable={false} />
      <dock-widget parent:area="right" window-title="Inspector">
        <label text="dock widget on the right" />
      </dock-widget>
      <label text="central widget" />
    </main-window>
  {/if}
</box-layout>
