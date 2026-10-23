<script>
  // FX6b: a change made by a timer, which must render with no host call into the root.
  let later = $state(0);
</script>

<box>
  <label>Later {later}</label>
  <button onclicked={() => setTimeout(() => later++, 0)}>Start timer</button>
</box>
