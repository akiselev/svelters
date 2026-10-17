<!-- Qt: QMainWindow slots. statusbar is routed by kind (no slot= needed), the label goes to
     the central slot (default). GTK has no statusbar element. -->
<script>
  let n = $state(0);
</script>

<window title="Main window">
  <box orientation="vertical">
    <label>Clicked {n} times</label>
    <button onclick={() => n++}>Click</button>
  </box>
  <statusbar />
</window>
