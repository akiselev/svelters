<frame frame-style="window" padding={12} gap={12} focus-scope="dialog">
  <label font-weight={600}>Delete 12 footprints?</label>
  <label>This cannot be undone.</label>
  <box flex-direction="row" gap={6} justify-content="end">
    <button text="Keep" default cancel dialog-role="reject" />
    <button anchor="subject" text="Delete" dialog-role="destructive" />
  </box>
</frame>
