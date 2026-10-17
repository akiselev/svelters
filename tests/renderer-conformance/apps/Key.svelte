<script>
  let n = $state(0);
  export function inc() { n++; }
</script>
<box>{#key n}<k>{n}</k>{/key}</box>
