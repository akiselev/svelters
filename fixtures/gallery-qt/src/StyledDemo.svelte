<script>
  // The styling demo shared by the "styling" and "styling dark" pages: scoped component styles,
  // class:, style:, --sv-* tokens. The dark page only switches the colour scheme and a token.
  // The last row uses setter properties (written with setProp / setChildAttr, not CSS): the
  // names are the toolkit's own, so each gallery has its own row.
  let count = $state(0);
  let warn = $state(false);
</script>

<box-layout class="stage" direction="top-to-bottom" spacing="12">
  <box-layout class="card" direction="top-to-bottom" spacing="8">
    <label class="heading">Styled card</label>
    <label class="muted">Colours come from --sv-* tokens.</label>
    <label class="count" class:warn style:font-weight={count > 2 ? 'bold' : null}>Count: {count}</label>
    <box-layout direction="left-to-right" spacing="8">
      <push-button class="primary" onclicked={() => count++}>Increment</push-button>
      <push-button class="ghost" class:active={warn} onclicked={() => (warn = !warn)}>Toggle warning</push-button>
    </box-layout>
    <line-edit class="field" />
    <box-layout class="setters" class:dense={warn}>
      <label class="caption">Setters</label>
      <push-button class="grow">stretch</push-button>
      <push-button style="minimum-width: 120px; maximum-width: 160px">minimum-width</push-button>
    </box-layout>
  </box-layout>
</box-layout>

<style>
  .stage { background-color: var(--sv-bg); padding: var(--sv-space-4); min-width: 480px; min-height: 260px; }
  .card { background-color: var(--sv-surface); border: 1px solid var(--sv-border); border-radius: var(--sv-radius); padding: var(--sv-space-3); }
  .heading { color: var(--sv-fg); font-size: 15pt; font-weight: bold; }
  .muted { color: var(--sv-muted); }
  .count { color: var(--sv-fg); }
  .count.warn { color: var(--sv-danger); }
  push-button.primary { background-color: var(--sv-accent); color: var(--sv-accent-fg); border-radius: var(--sv-radius); padding: 6px 14px; }
  push-button.primary:hover { background-color: var(--sv-accent-hover); }
  push-button.ghost { color: var(--sv-fg); border: 1px solid var(--sv-border); border-radius: var(--sv-radius); padding: 6px 14px; }
  push-button.ghost.active { border-color: var(--sv-danger); }
  line-edit.field { color: var(--sv-fg); }
  line-edit.field:focus { border-color: var(--sv-accent); }
  .setters { spacing: 16; direction: left-to-right; contents-margins: 4 0 0 0; }
  .setters.dense { spacing: 2; }
  .grow { stretch: 1; }
  .caption { color: var(--sv-muted); }
</style>
