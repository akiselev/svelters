<script>
  // Every AppKit kind of the MAC1 slice on one page: `counter-mac --fixture gallery-appkit`.
  let text = $state('');
  let checked = $state(false);
  let value = $state(25);
  let clicks = $state(0);
  let slider;
  let check;
</script>

<window title="AppKit gallery" content-size={{ width: 420, height: 380 }}>
  <stack-view orientation="vertical" alignment="leading" spacing="10" edge-insets={{ top: 16, right: 16, bottom: 16, left: 16 }}>
    <text-field string-value="Labels are text-fields" editable={false} bezeled={false} draws-background={false} />
    <text-field
      placeholder-string="Type here"
      oncontrol-text-did-change={(t) => (text = t)}
    />
    <text-field string-value={`You typed: ${text}`} editable={false} bezeled={false} draws-background={false} />
    <secure-text-field placeholder-string="Password" />
    <stack-view orientation="horizontal" spacing="8">
      <button title="Click me" onaction={() => clicks++} />
      <text-field string-value={`${clicks} clicks`} editable={false} bezeled={false} draws-background={false} />
    </stack-view>
    <button
      bind:this={check}
      button-type="switch"
      title={checked ? 'Checked' : 'Unchecked'}
      onaction={() => (checked = check.getProp('state') === 'on')}
    />
    <slider
      bind:this={slider}
      min-value="0"
      max-value="100"
      double-value={value}
      onaction={() => (value = slider.getProp('double-value'))}
    />
    <progress-indicator indeterminate={false} min-value="0" max-value="100" double-value={value} />
    <box title="A box">
      <text-field string-value={`Slider: ${Math.round(value)}`} editable={false} bezeled={false} draws-background={false} />
    </box>
  </stack-view>
</window>
