<!-- kinds: QWidget QLineEdit -->
<script>
  // Event families (X8): every widget has synthetic signals raised by an event filter: key-release,
  // mouse-press/release/move, double-click, wheel, context-menu (sync: `e.preventDefault()` consumes the event
  // before the widget sees it) and focus-in/out, enter, leave, resize (posted). Payloads are plain values:
  // positions are {x, y} records, buttons and modifiers Qt enum masks. `on<signal>capture` on an ancestor sees
  // its descendants' events first.
  let keys = $state(0);
  let presses = $state(0);
  let pos = $state('-');
  let scrolls = $state(0);
  let focused = $state('no');
  let size = $state('-');
</script>

<box orientation="vertical" spacing={6}>
  <label>keys: {keys}</label>
  <label>presses: {presses}</label>
  <label>motion: {pos}</label>
  <label>scrolls: {scrolls}</label>
  <label>focus: {focused}</label>
  <label>size: {size}</label>
  <entry
    text="type here"
    onkey-press={(e, key, text) => {
      keys++;
      if (text === 'x') e.preventDefault(); // consumed: the entry never sees it
    }}
    onfocus-in={() => (focused = 'yes')}
    onfocus-out={() => (focused = 'no')}
  />
  <box
    orientation="vertical"
    spacing={4}
    onmouse-press={(e, position, button) => presses++}
    onmouse-move={(e, position) => (pos = `${Math.round(position.x)},${Math.round(position.y)}`)}
    onwheel={(e, position, delta) => { scrolls++; e.preventDefault(); }}
    onresize={(s) => (size = `${s.width}x${s.height}`)}
  >
    <label>click, move or scroll over this box</label>
  </box>
</box>
