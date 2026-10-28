<box padding={8} flex-grow={1}>
  <modal open heading="حذف البصمة؟">
    <box padding={12} gap={12} width={300}>
      <label>ستتم إزالة R12 ووسادتيها من اللوحة.</label>
      <dialog-buttons>
        <button text="إلغاء" dialog-role="reject" />
        <button anchor="subject" text="حذف" dialog-role="destructive" />
      </dialog-buttons>
    </box>
  </modal>
</box>
