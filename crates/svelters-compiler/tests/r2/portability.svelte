<!-- backends: gtk; portability: gtk,qt,appkit,winui -->
<script>
  let title = $state(''), visible = $state(true);
</script>
<window bind:title bind:visible>
  <window bind:title|nonreactive={title} />
</window>
