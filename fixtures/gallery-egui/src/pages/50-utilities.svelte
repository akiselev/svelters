<script>
  // U3 (plans/research/UTILITY-CSS.md): Tailwind-style utility classes on egui. Values are the
  // theme's tokens (`p-4` = --sv-space-4, `bg-accent` = --sv-accent), so they follow the scheme,
  // high contrast and app tokens; the host's style engine generates each rule on first use, even for
  // a dynamic `p-{n}`. Utilities are opt-in per embed: `svelters-egui-run --utilities` (or
  // `EmbedConfig::with_utilities`); without them these are plain classes. tests/gallery.rs opens
  // this page with utilities on.
  let n = $state(1);
</script>

<box class="gap-2">
  <box class="flex-row gap-2 items-center">
    <box class="p-2 bg-surface rounded"><label>p-2 bg-surface rounded</label></box>
    <box class="p-4 bg-list-hover rounded-md"><label>p-4 bg-list-hover rounded-md</label></box>
    <box class="p-{n} border rounded-lg"><label>p-{n} border rounded-lg</label></box>
  </box>
  <box class="flex-row gap-2 items-center">
    <button class="px-4 rounded-full hover:bg-list-hover" onclicked={() => (n = (n % 8) + 1)}>Padding {n}</button>
    <button class="shadow-2 hover:opacity-50">Opacity on hover</button>
  </box>
  <slider class="part-thumb:bg-accent part-fill:bg-list-hover" accessible-name="Accent thumb" />
</box>
