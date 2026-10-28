<box padding={8} flex-grow={1}>
  <modal open heading="The autorouter could not route every net on the selected layers">
    <box padding={12} gap={12} width={340}>
      <label>Nets VCC_3V3, GND and USB_D+ have no route within the clearance rules. Widen the board outline, allow vias on the inner layers, or route them by hand before running the design rule check again.</label>
      <dialog-buttons>
        <button text="Route by hand" dialog-role="reject" />
        <button anchor="subject" text="Open the rules" dialog-role="accept" />
      </dialog-buttons>
    </box>
  </modal>
</box>
