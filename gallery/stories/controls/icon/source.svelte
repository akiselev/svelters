<script>
  import gear from './gear.svg';
  let { tint = true, disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <icon anchor="subject" source={gear} {tint} size={24} accessible-name="Gear" enabled={!disabled} />
</box>
