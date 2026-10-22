<script>
  let log = $state('');
  const add = (s) => (log += s);
</script>

<box on:click|capture={() => add('c')} onclick={() => add('b')}>
  <button text={log} onclick={(e) => { add('t'); e.stopPropagation(); }} />
</box>
