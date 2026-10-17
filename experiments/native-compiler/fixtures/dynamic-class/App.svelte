<!-- Dynamic `class={expr}` against setter rules: the compiler turns every rule that needs a class
     the element does not have statically into a runtime condition on the element's class set
     (clsx semantics: strings, arrays, objects); class: directives still win for their class. The
     class string itself reaches the widget through host.style (X6). -->
<script>
  let mode = $state('primary');
  let dense = $state(false);
</script>

<window title="Classes">
  <box class={['toolbar', { dense }]}>
    <button class={mode}>Save</button>
    <button class="big {mode === 'primary' ? '' : 'primary'}" class:dense>Other</button>
    <button onclicked={() => (mode = mode === 'primary' ? 'plain' : 'primary')}>Mode</button>
    <button onclicked={() => (dense = !dense)}>Dense</button>
  </box>
</window>

<style>
  .toolbar {
    orientation: horizontal;
    spacing: 8px;
  }

  .toolbar.dense {
    spacing: 2px;
  }

  .primary {
    height-request: 36px;
    minimum-height: 36px;
  }

  .big.dense {
    height-request: 20px;
    minimum-height: 20px;
  }
</style>
