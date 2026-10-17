<!-- Dynamic `class={expr}` against native styles: the compiler turns every rule that needs a
     class the element does not have statically into a runtime condition on the element's class
     set (clsx semantics: strings, arrays, objects); class: directives still win for their class. -->
<script>
  let mode = $state('primary');
  let dense = $state(false);
</script>

<window title="Classes">
  <box class={['toolbar', { dense }]}>
    <button class={mode}>Save</button>
    <button class="big {mode === 'primary' ? '' : 'primary'}" class:big={dense}>Other</button>
    <button onclick={() => (mode = mode === 'primary' ? 'plain' : 'primary')}>Mode</button>
    <button onclick={() => (dense = !dense)}>Dense</button>
  </box>
</window>

<style>
  .toolbar {
    orientation: horizontal;
    spacing: 8dp;
  }

  .toolbar.dense {
    spacing: 2dp;
  }

  .primary {
    min-height: 36dp;
    emphasis: accent;
  }

  .dense .big {
    min-height: 20dp;
  }
</style>
