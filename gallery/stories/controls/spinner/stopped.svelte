<script>
  let { disabled = false } = $props();
</script>

<box padding={12} flex-direction="row" gap={6} align-items="center">
  <label>Before</label>
  <spinner anchor="subject" accessible-name="Idle" spinning={false} enabled={!disabled} />
  <label>After</label>
</box>
