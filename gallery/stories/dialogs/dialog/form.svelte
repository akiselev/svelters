<frame frame-style="window" padding={12} gap={8} focus-scope="dialog">
  <grid column-gap={8} row-gap={4} grid-template-columns="auto 1fr" align-items="center">
    <label for="net">Net name</label>
    <text-edit anchor="net" text="VCC_3V3" autofocus />
    <label>Class</label>
    <combo-box accessible-name="Class" options={['Default', 'Power', 'High speed']} selected={1} />
  </grid>
  <box flex-direction="row" gap={6} justify-content="end">
    <button text="Cancel" cancel dialog-role="reject" />
    <button anchor="subject" text="Apply" default dialog-role="accept" />
  </box>
</frame>
