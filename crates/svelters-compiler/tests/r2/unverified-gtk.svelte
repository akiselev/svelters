<!-- backends: gtk; strict: gtk -->
<script>
  // OV1: Gtk:Window:is-active is exact but only `connect`-verified (no window manager headless): it observes
  // with a warning; entry text is delivery-verified. (A weak fact with a signal plan keeps the plan: r2_observe.rs.)
  let f = $state(false), n = $state(0), s = $state('');
</script>
<window bind:is-active={f}>
  <box orientation="vertical">
    <entry bind:text={s} />
  </box>
</window>
