<!-- The shape of an app window: a toolbar row, a body that grows, a status line. -->
<window title="cadatomic">
  <box flex-direction="row" gap={4} padding={4} class="bar">
    <button icon="file.new" tooltip="New" />
    <button icon="file.open" tooltip="Open" />
    <button icon="file.save" tooltip="Save" />
    <separator orientation="vertical" />
    <button icon="run.start" tooltip="Run" />
  </box>
  <box flex-grow={1} padding={12} gap={6}>
    <label text="Body" />
    <label text="Grows to fill the window." />
  </box>
  <box flex-direction="row" padding={4} class="status">
    <label text="Ready" />
  </box>
</window>

<style>
  .bar {
    background-color: var(--vscode-sideBar-background);
  }
  .status {
    background-color: var(--vscode-statusBar-background);
    color: var(--vscode-statusBar-foreground);
  }
</style>
