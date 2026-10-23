<script>
  // FX6b: a store-backed panel (a `createSubscriber` source, as cadatomic's store fields and
  // `svelte/reactivity` read) whose derived never returns once `stall` is set: the turn budget stops
  // the flush, the embed faults the root and disposes it (the subscriber's last unsubscribe runs then).
  import { createSubscriber } from 'svelte/reactivity';
  let { stall = false } = $props();
  let version = 0;
  const subscribe = createSubscriber(() => () => version++);
  const store = {
    get value() {
      subscribe();
      return version;
    },
  };
  const shown = $derived.by(() => {
    if (stall) for (;;) {}
    return `Store ${store.value}`;
  });
</script>

<label>{shown}</label>
