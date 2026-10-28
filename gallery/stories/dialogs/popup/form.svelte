<box padding={12} flex-grow={1} align-items="start" gap={4}>
  <button text="Track width" />
  <popup anchor="subject" open placement="bottom-start">
    <box padding={10} gap={6} width={220}>
      <label>Track width for the selected nets</label>
      <text-edit text="0.25 mm" accessible-name="Width" />
      <sides>
        <button text="Cancel" dialog-role="reject" />
        <button text="Apply" dialog-role="accept" />
      </sides>
    </box>
  </popup>
</box>
