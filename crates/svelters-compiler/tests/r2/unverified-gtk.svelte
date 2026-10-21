<!-- backends: gtk; strict: gtk -->
<script>
  // OV1: `exact` facts that no generated row has seen deliver a native change (verified: weak / none)
  let f = $state(false), n = $state(0), s = $state('');
</script>
<window bind:is-active={f}>
  <box orientation="vertical">
    <notebook bind:page={n} />
    <entry bind:text={s} />
  </box>
</window>
