<!-- backends: appkit -->
<script>
  let s = $state(''), n = $state(0), v = $state(0);
</script>
<window>
  <stack-view>
    <text-field bind:string-value={s} />
    <text-field bind:int-value={n} />
    <slider bind:double-value={v} />
  </stack-view>
</window>
