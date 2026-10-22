<script>
  // WG6: widget polish. Tooltips on disabled widgets and the `tooltip` element, `status-tip`,
  // `elide`, `scroll-into-view`, `cursor`, `key-sequence-edit`, combo type-ahead, file drops.
  let shortcut = $state('Ctrl+Shift+K');
  let material = $state(0);
  let dropped = $state('Drop files from the file manager here');
  let action = $state('');
  let last = $state();
</script>

<box class="col">
  <box class="row">
    <button text="Fillet" enabled={false} tooltip="Select an edge first" status-tip="Rounds the selected edges" />
    <button text="Extrude" status-tip="Extrudes a sketch profile">
      <tooltip>
        <label font-weight={700}>Extrude</label>
        <label>Pulls a closed sketch profile into a solid.</label>
      </tooltip>
    </button>
  </box>
  <label elide="middle" width={220}>/home/user/projects/cadatomic/boards/main/power-supply.kicad_pcb</label>
  <box class="row">
    <label cursor="pointer">Pointer</label>
    <label cursor="ew-resize">Resize</label>
    <label cursor="crosshair">Crosshair</label>
    <label cursor="not-allowed">Not allowed</label>
  </box>
  <box class="row">
    <label>Shortcut</label>
    <key-sequence-edit bind:value={shortcut} accessible-name="Shortcut" />
    <label>{shortcut || 'none'}</label>
  </box>
  <box class="row">
    <label>Material (type to jump)</label>
    <combo-box options={['Aluminium', 'Brass', 'Bronze', 'Copper', 'Steel', 'Titanium']} bind:selected={material} />
  </box>
  <frame
    drop-types="text/uri-list cad/part"
    ondrop={(type, data, x, y, how) => ((dropped = data), (action = how))}
    height={60}
  >
    <label>{dropped} {action}</label>
  </frame>
  <box class="row">
    <button text="Scroll to the last row" onclicked={() => last.call('scroll-into-view')} />
  </box>
  <scroll-area height={100}>
    <box>
      {#each { length: 30 } as _, k}
        {#if k === 29}
          <label bind:this={last}>Row {k} (the last)</label>
        {:else}
          <label>Row {k}</label>
        {/if}
      {/each}
    </box>
  </scroll-area>
</box>

<style>
  .col {
    gap: 8px;
  }
  .row {
    flex-direction: row;
    gap: 8px;
  }
</style>
