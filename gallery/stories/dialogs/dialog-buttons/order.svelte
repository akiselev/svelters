<script>
  let { order = 'windows' } = $props();
</script>

<box padding={12}>
  <dialog-buttons {order}>
    <button text="Help" dialog-role="help" />
    <button text="Cancel" dialog-role="reject" />
    <button text="Apply" dialog-role="apply" />
    <button anchor="subject" text="OK" dialog-role="accept" default />
  </dialog-buttons>
</box>
