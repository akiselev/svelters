<script>
  let { columns = 'auto 1fr' } = $props();
</script>

<grid anchor="subject" grid-template-columns={columns} gap={4} padding={8}>
  <frame frame-style="group"><label>A</label></frame>
  <frame frame-style="group"><label>Bee</label></frame>
  <frame frame-style="group"><label>Sea</label></frame>
  <frame frame-style="group"><label>Dee</label></frame>
  <frame frame-style="group"><label>E</label></frame>
  <frame frame-style="group"><label>Eff</label></frame>
</grid>
