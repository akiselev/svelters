<script>
  // CX3: native links. `form` is a Rust store (crates/svelters-egui/tests/bind_targets.rs), so each
  // `bind:` below is linked in the host: typing and Rust writes cross without JS. Nothing here reads
  // `form.*` in the template, so no change of either field is delivered to JS.
  import { form } from 'svelters:app/cx3';
</script>

<window title="Link" default-width={360} default-height={200}>
  <box>
    <text-edit bind:text={form.title} accessible-name="title" />
    <slider bind:value={form.speed} min={0} max={10} accessible-name="speed" />
  </box>
</window>
