<!-- kinds: Button ToggleButton CheckButton Switch LinkButton LockButton MenuButton ScaleButton VolumeButton -->
<script>
  let clicks = $state(0);
  let toggled = $state(false);
  let checked = $state(false);
  let on = $state(false);
  let toggledEchoes = $state(0);
  let checkedEchoes = $state(0);
  let switchEchoes = $state(0);
  let linkEvents = $state(0);
  $effect(() => {
    // programmatic writes after mount: must not come back as change events
    toggled = true;
    checked = true;
    on = true;
  });
</script>

<box orientation="vertical" spacing="6">
  <box orientation="horizontal" spacing="6">
    <button label="Clicked {clicks}" has-frame={true} use-underline={false} onclicked={() => clicks++} />
    <toggle-button label="Toggle {toggled}" bind:active={toggled} ontoggled={() => toggledEchoes++} />
    <label label="echo toggle: {toggledEchoes}" />
  </box>
  <box orientation="horizontal" spacing="6">
    <check-button label="Check {checked}" bind:active={checked} ontoggled={() => checkedEchoes++} />
    <check-button label="Mixed" inconsistent={true} />
    <label label="echo check: {checkedEchoes}" />
  </box>
  <box orientation="horizontal" spacing="6">
    <switch bind:active={() => on, (v) => { on = v; switchEchoes++; }} />
    <label label="echo switch: {switchEchoes}" />
  </box>
  <box orientation="horizontal" spacing="6">
    <link-button uri="https://example.org" label="Link" onactivate-link={() => { linkEvents++; }} />
    <lock-button />
    <menu-button label="Menu" direction="down" always-show-arrow={true}>
      <popover autohide={true}>
        <label label="popover content" />
      </popover>
    </menu-button>
  </box>
  <box orientation="horizontal" spacing="6">
    <scale-button />
    <volume-button />
  </box>
</box>
