<script>
  let { value = $bindable(0) } = $props();
  let count = $state(0);
  let raw = $state.raw(1);
  let plain = 1;
  export function reset() {
    count = 0;
    raw = 2;
    plain = 3;
  }
  export const VERSION = '1.0';
  export { count, raw as rawValue, plain, value };
</script>

<label label={`${count} ${value}`} />
