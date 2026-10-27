<frame frame-style="window" padding={12} gap={12} focus-scope="dialog">
  <label font-weight={600}>Save changes to “main.kicad_pcb”?</label>
  <label>Your changes are lost if you close without saving.</label>
  <box flex-direction="row" gap={6} justify-content="end">
    <button text="Cancel" cancel dialog-role="reject" />
    <button anchor="subject" text="Save" default dialog-role="accept" />
  </box>
</frame>
