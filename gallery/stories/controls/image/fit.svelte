<script>
  import landscape from './landscape.svg';
  let { fit = 'contain' } = $props();
</script>

<box padding={8} align-items="start">
  <image anchor="subject" source={landscape} {fit} width={100} height={100} accessible-name="Landscape" />
</box>
