<script>
  // CX3: draft bindings. `slot` is a bind source the spec fragment declares (bind-sources.json), so
  // `bind:text={slot(path)}` reads `draftValue(path)` and writes through `setDraft(path, value)`.
  import { slot, drafts } from '#drafts';
</script>

<window title="Draft" default-width={400} default-height={240}>
  <box>
    <text-edit bind:text|commit={slot('depth')} accessible-name="depth" />
    <text-edit bind:text={slot('width')} accessible-name="width" />
    <label>Log: {drafts.log.join(' ')}</label>
  </box>
</window>
