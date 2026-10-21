<!-- backends: gtk; strict: gtk -->
<script>
  // E3: a writable field of a Rust store (`svelters:app/*`) as the bound value. The lowering is the
  // element prop's (Gtk.Range:value and Gtk.SpinButton:value are exact + delivery-verified, Gtk.Entry:text
  // keeps its declared signal plan as the fallback); the store side needs only an assignable member
  // expression: its getter is reactive (`observed`) and its setter goes through `setProp`.
  import { telemetry } from 'svelters:app/telemetry';
</script>

<window>
  <box>
    <scale lower={0} upper={20} bind:value={telemetry.speed} />
    <spin-button lower={0} upper={20} bind:value={telemetry.speed} />
    <entry bind:text={telemetry.label} />
  </box>
</window>
