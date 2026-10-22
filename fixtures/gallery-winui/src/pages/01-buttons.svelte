<!-- kinds: Button ToggleButton CheckBox RadioButton RepeatButton HyperlinkButton DropDownButton SplitButton ToggleSwitch MenuFlyout MenuFlyoutItem -->
<script>
  let clicks = $state(0);
  let repeats = $state(0);
  let toggled = $state(false);
  let picked = $state('none');
  let on = $state(false);
</script>

<stack-panel orientation="vertical" spacing="6">
  <button onclick={() => clicks++}>Button (clicks: {clicks})</button>
  <toggle-button is-checked={toggled} onchecked={() => (toggled = true)} onunchecked={() => (toggled = false)}>Toggle button: {toggled ? 'on' : 'off'}</toggle-button>
  <check-box>Check box</check-box>
  <radio-button group-name="gallery">Radio one</radio-button>
  <radio-button group-name="gallery">Radio two</radio-button>
  <repeat-button delay={300} interval={100} onclick={() => repeats++}>Repeat button ({repeats})</repeat-button>
  <hyperlink-button>Hyperlink button</hyperlink-button>
  <drop-down-button content="Drop-down button: {picked}">
    <menu-flyout slot="flyout">
      <menu-flyout-item text="First" onclick={() => (picked = 'first')} />
      <menu-flyout-item text="Second" onclick={() => (picked = 'second')} />
    </menu-flyout>
  </drop-down-button>
  <split-button content="Split button">
    <menu-flyout slot="flyout">
      <menu-flyout-item text="Option" onclick={() => (picked = 'split option')} />
    </menu-flyout>
  </split-button>
  <toggle-switch header="Toggle switch" on-content="On" off-content="Off" is-on={on} ontoggled={() => (on = !on)} />
</stack-panel>
