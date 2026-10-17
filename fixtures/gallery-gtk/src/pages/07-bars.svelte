<!-- kinds: HeaderBar WindowControls Statusbar InfoBar PopoverMenuBar -->
<box orientation="vertical" spacing="8">
  <header-bar show-title-buttons={false} decoration-layout="menu:close" use-native-controls={false} title="bar title">
    <button label="start" />
  </header-bar>
  <box orientation="horizontal" spacing="6">
    <label label="controls (start)" />
    <window-controls side="start" decoration-layout="close,minimize:maximize" use-native-controls={false} />
  </box>
  <box orientation="horizontal" spacing="6">
    <label label="controls (end)" />
    <window-controls side="end" decoration-layout="menu:minimize,maximize,close" />
  </box>
  <label label="Statusbar and InfoBar are deprecated and not exposed" />
</box>
