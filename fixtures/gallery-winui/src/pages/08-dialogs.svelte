<!-- kinds: ContentDialog TeachingTip -->
<script>
  let dialog = $state(null);
  let tip = $state(false);
  let result = $state('not shown');
</script>

<stack-panel orientation="vertical" spacing="6">
  <button onclick={() => dialog?.call('show-async')}>Show content dialog</button>
  <text-block>Dialog: {result}</text-block>
  <content-dialog bind:this={dialog} title="Content dialog" primary-button-text="OK" close-button-text="Cancel" default-button="primary"
    onopened={() => (result = 'open')} onclosed={(r) => (result = `closed (${r})`)}>
    <text-block>A modal dialog in the window's XAML root.</text-block>
  </content-dialog>
  <button onclick={() => (tip = !tip)}>Toggle teaching tip</button>
  <teaching-tip title="Teaching tip" subtitle="A non-modal tip" is-open={tip} />
</stack-panel>
