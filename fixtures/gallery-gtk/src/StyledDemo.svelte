<script>
  // The styling demo shared by the "styling" and "styling dark" pages: scoped component styles,
  // class:, style:, --sv-* tokens. The dark page only switches the colour scheme and a token.
  // The last row uses setter properties (written with setProp / setChildAttr, not CSS): the
  // names are the toolkit's own, so each gallery has its own row.
  let count = $state(0);
  let warn = $state(false);
</script>

<box class="stage" orientation="vertical" spacing="12">
  <box class="card" orientation="vertical" spacing="8">
    <label class="heading">Styled card</label>
    <label class="muted">Colours come from --sv-* tokens.</label>
    <label class="count" class:warn style:font-weight={count > 2 ? 'bold' : null}>Count: {count}</label>
    <box orientation="horizontal" spacing="8">
      <button class="primary" onclick={() => count++}>Increment</button>
      <button class="ghost" class:active={warn} onclick={() => (warn = !warn)}>Toggle warning</button>
    </box>
    <entry class="field" />
    <box class="setters" class:dense={warn}>
      <label class="caption" style="valign: center">Setters</label>
      <button class="grow">hexpand</button>
      <button style="width-request: 120px">width-request</button>
    </box>
  </box>
</box>

<style>
  .stage { background-color: var(--sv-bg); padding: var(--sv-space-4); min-width: 480px; min-height: 260px; }
  .card { background-color: var(--sv-surface); border: 1px solid var(--sv-border); border-radius: var(--sv-radius); padding: var(--sv-space-3); }
  .heading { color: var(--sv-fg); font-size: 15pt; font-weight: bold; }
  .muted { color: var(--sv-muted); }
  .count { color: var(--sv-fg); }
  .count.warn { color: var(--sv-danger); }
  button.primary { background-color: var(--sv-accent); color: var(--sv-accent-fg); border-radius: var(--sv-radius); padding: 6px 14px; }
  button.primary:hover { background-color: var(--sv-accent-hover); }
  button.ghost { color: var(--sv-fg); border: 1px solid var(--sv-border); border-radius: var(--sv-radius); padding: 6px 14px; }
  button.ghost.active { border-color: var(--sv-danger); }
  entry.field { color: var(--sv-fg); }
  entry.field:focus { border-color: var(--sv-accent); }
  .setters { spacing: 16px; orientation: horizontal; margin-top: 4px; }
  .setters.dense { spacing: 2px; }
  .grow { hexpand: true; }
  .caption { color: var(--sv-muted); }
</style>
