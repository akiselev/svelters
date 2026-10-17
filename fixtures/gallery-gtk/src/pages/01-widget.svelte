<!-- kinds: Widget Editable Orientable Actionable -->
<script>
  let shown = $state(true);
  let clicks = $state(0);
</script>

<box orientation="vertical" spacing="6" margin-start={12} margin-end={12} margin-top={8}>
  <label label="Plain label" tooltip-text="a tooltip" />
  <label label="Centered, 50% opacity" halign="center" opacity={0.5} />
  <label label="Insensitive label" sensitive={false} />
  <label label="End aligned" halign="end" hexpand={true} />
  <label label="Toggled by the button" visible={shown} onshow={() => {}} onhide={() => {}} />
  <button label="Toggle label" onclicked={() => { shown = !shown; clicks++; }} />
  <button label="Insensitive button" sensitive={false} />
  <button label="Action button" action-name="win.close" tooltip-markup="<b>bold</b> tip" />
  <label label="clicks: {clicks}" name="clicks" width-request={160} height-request={24} />
  <entry editable={false} width-chars={12} max-width-chars={20} xalign={0.5} enable-undo={true} />
</box>
