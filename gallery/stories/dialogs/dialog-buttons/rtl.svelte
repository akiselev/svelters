<script>
  let { order = 'windows' } = $props();
</script>

<box dir="rtl" padding={12}>
  <dialog-buttons {order}>
    <button text="مساعدة" dialog-role="help" />
    <button text="إلغاء" dialog-role="reject" />
    <button text="تطبيق" dialog-role="apply" />
    <button anchor="subject" text="موافق" dialog-role="accept" default />
  </dialog-buttons>
</box>
