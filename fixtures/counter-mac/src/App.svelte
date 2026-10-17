<script>
  // The counter on AppKit: tags are AppKit class names without `NS` (NSStackView -> stack-view),
  // props and events are native names in kebab-case. A label is a non-editable text-field.
  let count = $state(0);
</script>

<window title="Counter">
  <!-- No object-valued props here: the host's op-log fixture test replays this on the log
       backend, which has no descriptors to encode them (the gallery sets edge-insets). -->
  <stack-view orientation="vertical" spacing="8">
    <text-field
      string-value={`Count: ${count}`}
      editable={false}
      selectable={false}
      bezeled={false}
      draws-background={false}
    />
    <button title="Increment" onaction={() => count++} />
  </stack-view>
</window>
