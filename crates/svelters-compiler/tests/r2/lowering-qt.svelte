<!-- backends: qt -->
<script>
  let text = $state(''), plain = $state(''), checked = $state(false), size = $state(null), size0 = $state(null), once = $state('');
</script>
<main-window>
  <box-layout direction="top-to-bottom" bind:size>
    <box-layout bind:size|nonreactive={size0} />
    <line-edit bind:text />
    <line-edit bind:text|nonreactive={once} />
    <text-edit bind:plain-text={plain} />
    <group-box title="g" checkable={true} bind:checked />
  </box-layout>
</main-window>
