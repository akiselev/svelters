<box flex-grow={1} flex-direction="row">
  <panel anchor="subject" side="left" size={140} separator={false}>
    <box padding={8}><label>Outline</label></box>
  </panel>
  <box class="editor" padding={8} flex-grow={1}><label>Board</label></box>
</box>

<style>
  .editor {
    background-color: var(--sv-surface);
  }
</style>
