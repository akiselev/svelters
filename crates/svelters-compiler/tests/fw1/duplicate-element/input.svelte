<script>
  function h() {}
</script>

<box>
  <button on:clicked onclicked={h} />
  <button on:clicked={h} on:clicked />
  <button on:clicked={h} onclicked={h} />
</box>
