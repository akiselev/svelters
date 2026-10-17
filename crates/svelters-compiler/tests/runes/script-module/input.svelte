<script module>
  import { untrack } from 'svelte';
  let instances = $state(0);
  export const shared = $state({ total: 0 });
  export function count() {
    return instances;
  }
</script>

<script>
  instances++;
  shared.total += 1;
  let local = $derived(instances * 2);
</script>

<label label={`${instances} ${shared.total} ${local} ${untrack(() => 1)}`} />
