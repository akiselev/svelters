<script>
  let count = $state(0);
  export let legacy = 1;
  export const twice = $derived(count * 2);
  export default {};
</script>

<label label="x" />
