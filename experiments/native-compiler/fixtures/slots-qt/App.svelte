<!-- Qt: QMainWindow slots. statusbar is routed by kind to the statusbar slot (no slot= needed),
     the box goes to the central slot (default). GTK has a (deprecated) statusbar element, but
     GtkWindow has a single `child` slot, so GTK rejects this at compile time (slot_cardinality). -->
<script>
  let n = $state(0);
</script>

<main-window window-title="Main window">
  <box-layout direction="top-to-bottom">
    <label>Clicked {n} times</label>
    <push-button onclicked={() => n++}>Click</push-button>
  </box-layout>
  <status-bar />
</main-window>
