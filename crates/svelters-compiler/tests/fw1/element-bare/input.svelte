<script>
  // A toolbar that forwards its button's `clicked` to its parent (`<Toolbar onclicked={…}>`),
  // and keeps a local listener on another button (`on:clicked={h}` is a listener, as before).
  let count = $state(0);
</script>

<window>
  <box>
    <button label="Forwarded" on:clicked />
    <button label="Local" on:clicked={() => count++} />
    <window title="Closing" on:close-request />
  </box>
</window>
