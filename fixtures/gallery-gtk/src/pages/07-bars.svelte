<!-- kinds: HeaderBar WindowControls Statusbar InfoBar PopoverMenuBar -->
<box orientation="vertical" spacing="8">
  <headerbar showTitleButtons={false} decorationLayout="menu:close" useNativeControls={false} title="bar title">
    <button label="start" />
  </headerbar>
  <box orientation="horizontal" spacing="6">
    <label label="controls (start)" />
    <windowcontrols side="start" decorationLayout="close,minimize:maximize" useNativeControls={false} />
  </box>
  <box orientation="horizontal" spacing="6">
    <label label="controls (end)" />
    <windowcontrols side="end" decorationLayout="menu:minimize,maximize,close" />
  </box>
  <label label="Statusbar and InfoBar are deprecated and not exposed" />
</box>
