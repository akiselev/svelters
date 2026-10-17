<script>
  let n = $state(1);
  let log = [];
  export function inc() { n++; }
  export function getLog() { return log; }
  function act(node, param) { log.push('action mount ' + param); return { update(p) { log.push('action update ' + p); }, destroy() { log.push('action destroy'); } }; }
  const attachment = (node) => { log.push('attach n=' + n + ' tag=' + node.tag); return () => log.push('detach n=' + n); };
</script>
<widget use:act={n} {@attach attachment} />
