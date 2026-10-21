<!-- backends: winui -->
<script>
  let s = $state(''), w = $state(0), c = $state(false);
</script>
<window>
  <stack-panel bind:actual-width={w}>
    <text-box bind:text={s} />
    <check-box bind:is-checked={c} />
  </stack-panel>
</window>
