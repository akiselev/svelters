<!-- kinds: Button CheckBox RadioButton Switch ToggleButton TextEdit ComboBox Slider ProgressBar CollapsingHeader TabView ScrollArea SplitView Label -->
<script>
  // VIS1c (plans/milestones/VIS1c-visual-gate.md): every kind the VS Code references cover, in its
  // static states (rest, disabled, checked/selected, invalid, filled); the gate drives hover,
  // pressed and focus. Each widget is found by its accessible name `<element>:<variant>` and
  // measured from svelters' PaintTrace (crates/svelters-testkit/tests/vis1.rs), then compared
  // with conformance/visual/vscode{,-macos}/<theme>/metrics.json. Button captions are the
  // references' captions, so widths compare (with a font tolerance: Segoe and SF are not ours).
  let tab = $state(0);
</script>

<box gap={10} padding={12} flex-grow={1}>
  <box flex-direction="row" gap={10} align-items="center">
    <button class="primary" accessible-name="button-primary:a">Reload and Enable Extensions</button>
    <button class="primary" accessible-name="button-primary:disabled" enabled={false}>Reload and Enable Extensions</button>
    <button class="primary" accessible-name="button-primary-dialog:a">Save</button>
    <button accessible-name="button-secondary:a">Don't Save</button>
    <button accessible-name="button-secondary:disabled" enabled={false}>Don't Save</button>
  </box>
  <box flex-direction="row" gap={14} align-items="center">
    <checkbox accessible-name="checkbox:a" text="Enabled" />
    <checkbox accessible-name="checkbox:checked" text="Enabled" checked />
    <checkbox accessible-name="checkbox:disabled" text="Enabled" enabled={false} />
    <radio-button accessible-name="radio:a" text="Alpha" />
    <radio-button accessible-name="radio:checked" text="Beta" checked />
    <switch accessible-name="switch:a" text="Wi-Fi" />
    <switch accessible-name="switch:checked" text="Wi-Fi" active />
    <toggle-button accessible-name="toggle:a" text="Aa" />
    <toggle-button accessible-name="toggle:checked" text="Aa" checked />
  </box>
  <box flex-direction="row" gap={10} align-items="center">
    <text-edit accessible-name="input:a" hint-text="Find" width={216} />
    <text-edit accessible-name="input:filled" text="line 1" width={216} />
    <text-edit accessible-name="input:invalid" text="(" invalid width={216} />
    <text-edit accessible-name="input:disabled" text="120" enabled={false} width={200} />
  </box>
  <box flex-direction="row" gap={10} align-items="center">
    <combo-box accessible-name="select:a" options={['none', 'mouseover', 'scroll']} selected={0} width={320} />
    <combo-box accessible-name="select:disabled" options={['none', 'mouseover', 'scroll']} selected={0} enabled={false} width={320} />
  </box>
  <box flex-direction="row" gap={10} align-items="center">
    <slider accessible-name="slider:a" value={40} show-value={false} width={200} />
    <progress-bar accessible-name="progress:a" fraction={0.5} width={200} />
    <label accessible-name="label:a">Limit the width of the minimap.</label>
  </box>
  <box flex-direction="row" gap={16}>
    <box width={300} gap={0}>
      <collapsing-header accessible-name="pane-header:a" text="WS" open={false} />
    </box>
    <tab-view bind:selected={tab} width={360} height={90}>
      <tab title="long.txt" accessible-name="tab:active" closable><label>first</label></tab>
      <tab title="main.ts" accessible-name="tab:inactive" closable><label>second</label></tab>
    </tab-view>
  </box>
  <box flex-direction="row" gap={16} height={140}>
    <scroll-area accessible-name="scrollbar:a" width={200} height={120} gap={2}>
      {#each Array.from({ length: 40 }, (_, i) => i + 1) as i}
        <label>line {i}</label>
      {/each}
    </scroll-area>
    <split-view accessible-name="sash:a" width={300} height={120}>
      <label>left</label>
      <label>right</label>
    </split-view>
  </box>
</box>
