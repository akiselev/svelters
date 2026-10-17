<!-- kinds: EventControllerKey GestureClick EventControllerMotion EventControllerScroll EventControllerFocus -->
<script>
  // Event controllers are declarative children (X8): a controller element inside a widget is attached to it
  // (gtk_widget_add_controller) and removed with it. `key-pressed` and `scroll` are sync signals: the handler
  // decides through the event object (`e.preventDefault()` consumes the event, `e.result = false` lets it
  // through). A controller's `on<signal>capture` puts it in the capture phase. The scroll controller only
  // reports scrolls for the directions in its `flags`.
  let keys = $state(0);
  let last = $state('none');
  let presses = $state(0);
  let pos = $state('-');
  let scrolls = $state(0);
  let focused = $state('no');
</script>

<box orientation="vertical" spacing={6}>
  <label>keys: {keys} (last {last})</label>
  <label>presses: {presses}</label>
  <label>motion: {pos}</label>
  <label>scrolls: {scrolls}</label>
  <label>focus: {focused}</label>
  <entry text="type here">
    <keycontroller
      onkey-pressed={(e, keyval) => {
        keys++;
        last = String(keyval);
        if (keyval === 120) e.preventDefault(); // 'x' is consumed: the entry never sees it
      }}
    />
    <focuscontroller onenter={() => (focused = 'yes')} onleave={() => (focused = 'no')} />
  </entry>
  <box orientation="vertical" spacing={4}>
    <label>click, move or scroll over this box</label>
    <clickgesture onpressed={(n, x, y) => presses++} />
    <motioncontroller onmotion={(x, y) => (pos = `${Math.round(x)},${Math.round(y)}`)} />
    <scrollcontroller flags="vertical" onscroll={(e, dx, dy) => { scrolls++; e.preventDefault(); }} />
  </box>
</box>
