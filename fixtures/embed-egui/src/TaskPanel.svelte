<script>
  // WG1: a task-panel-shaped form (cadatomic's SC1 panel, body.box): a grid of labelled fields, an
  // invalid field, a combo box, a segmented toggle group, a collapsed "Placement" group, an icon
  // button with a tooltip, all bound to local state that a summary label echoes.
  // crates/svelters-egui/tests/widgets_panel.rs drives it.
  const dirs = ['+X', '+Y', '+Z'];
  let width = $state('10 mm');
  let height = $state('');
  let plane = $state(0);
  let dir = $state('+Z');
  let placement = $state(false);
  let offset = $state('0');
  let snap = $state(false);
  let settings = $state(0);
  const heightValid = $derived(/^\d+(\.\d+)?( ?mm)?$/.test(height.trim()));
</script>

<scroll-area flex-grow={1} gap={6} padding={4}>
  <grid column-gap={8} row-gap={4} align-items="center">
    <label>Width</label>
    <text-edit hint-text="Width" bind:text={width} />
    <label>Height</label>
    <text-edit hint-text="Height" bind:text={height} invalid={!heightValid} tooltip="A length, e.g. 20 mm" />
    <label>Plane</label>
    <combo-box accessible-name="Plane" options={['XY', 'YZ', 'ZX']} bind:selected={plane} />
    <label>Direction</label>
    <box flex-direction="row">
      {#each dirs as d (d)}
        <toggle-button radio text={d} checked={dir === d} onchanged={() => (dir = d)} />
      {/each}
    </box>
  </grid>
  <separator />
  <collapsing-header text="Placement" bind:open={placement}>
    <grid column-gap={8}>
      <label>Offset</label>
      <text-edit hint-text="Offset" bind:text={offset} />
    </grid>
  </collapsing-header>
  <box flex-direction="row" gap={4}>
    <icon name="gear" frame tooltip="Settings" onclicked={() => settings++} />
    <toggle-button text="Snap" bind:checked={snap} />
  </box>
  <label
    >Summary: {width} x {height} valid {heightValid} plane {plane} dir {dir} placement {placement} offset {offset} snap {snap} settings {settings}</label
  >
</scroll-area>
