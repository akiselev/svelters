<box padding={8} flex-grow={1}>
  <label>Board editor</label>
  <modal open heading="Delete footprint?">
    <box gap={12} width={300}>
      <label>R12 and its 2 pads will be removed from the board.</label>
      <dialog-buttons>
        <button text="Cancel" dialog-role="reject" />
        <button anchor="subject" text="Delete" dialog-role="destructive" />
      </dialog-buttons>
    </box>
  </modal>
</box>
