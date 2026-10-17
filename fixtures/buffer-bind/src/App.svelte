<script>
  // bind:text on a text view goes through its buffer, bind:value on a scale through its adjustment
  // (X7, G12): both are forwarded by the GTK backend.
  let text = $state('start');
  let value = $state(10);
</script>

<window title="buffer-bind">
  <box orientation="vertical" spacing="6">
    <textview bind:text />
    <label>{`text=${text}`}</label>
    <scale bind:value lower={0} upper={100} stepIncrement={1} />
    <label>{`value=${value}`}</label>
    <button onclick={() => { text = 'from js'; value = 55; }}>Set</button>
  </box>
</window>
