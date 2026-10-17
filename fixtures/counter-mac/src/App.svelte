<script>
  // The counter on AppKit: tags are AppKit class names without `NS` (NSStackView -> stack-view),
  // props and events are native names in kebab-case. A label is a non-editable text-field.
  let count = $state(0);
</script>

<window title="Counter">
  <stack-view orientation="vertical" spacing="8" edge-insets={{ top: 20, right: 20, bottom: 20, left: 20 }}>
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
