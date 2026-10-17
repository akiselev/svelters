<script>
  let on = $state(false);
  export function toggle() { on = !on; }
</script>
<widget class:active={on} style:color={on ? 'red' : 'blue'} />
