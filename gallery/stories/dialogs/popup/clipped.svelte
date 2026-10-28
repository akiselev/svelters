<!-- The popup's owner is inside a short scroll area, and a panel comes after it: the popup draws whole, above both. -->
<box padding={12} flex-grow={1} gap={8}>
  <scroll-area height={48} width={200}>
    <box padding={6} align-items="start">
      <box>
        <button text="Net class" />
        <popup anchor="subject" open placement="bottom-start">
          <box padding={8} gap={4}>
            <label>Default</label>
            <label>Power</label>
            <label>High speed</label>
          </box>
        </popup>
      </box>
    </box>
  </scroll-area>
  <box class="panel" padding={8} flex-grow={1}>
    <label>Properties</label>
  </box>
</box>

<style>
  .panel {
    background-color: var(--sv-surface);
  }
</style>
