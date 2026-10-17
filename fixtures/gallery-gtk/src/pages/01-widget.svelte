<!-- kinds: Widget Editable Orientable Actionable -->
<script>
  let shown = $state(true);
  let clicks = $state(0);
</script>

<box orientation="vertical" spacing="6" marginStart={12} marginEnd={12} marginTop={8}>
  <label label="Plain label" tooltipText="a tooltip" />
  <label label="Centered, 50% opacity" halign="center" opacity={0.5} />
  <label label="Insensitive label" sensitive={false} />
  <label label="End aligned" halign="end" hexpand={true} />
  <label label="Toggled by the button" visible={shown} onshow={() => {}} onhide={() => {}} />
  <button label="Toggle label" onclick={() => { shown = !shown; clicks++; }} />
  <button label="Insensitive button" sensitive={false} />
  <button label="Action button" actionName="win.close" tooltipMarkup="<b>bold</b> tip" />
  <label label="clicks: {clicks}" name="clicks" widthRequest={160} heightRequest={24} />
  <entry editable={false} widthChars={12} maxWidthChars={20} xalign={0.5} enableUndo={true} />
</box>
