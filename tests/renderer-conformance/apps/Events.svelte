<script>
  let seen = [];
  export function getSeen() { return seen; }
  function onclose(...args) { seen.push({ argc: args.length, args: args.map(a => typeof a === 'object' && a ? Object.keys(a) : a) }); return true; }
  function onkey(e) { e?.preventDefault?.(); return 'handled'; }
</script>
<window onclose-request={onclose} onclick={onkey} onkeypress={onkey} onactivate={() => 42}></window>
