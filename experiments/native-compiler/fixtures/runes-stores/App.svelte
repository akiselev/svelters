<!-- Store auto-subscriptions: $store reads, assignments, compound assignments, updates and
     member mutations, plus a derived store. -->
<script>
  import { writable, derived } from 'svelte/store';
  const count = writable(1);
  const user = writable({ name: 'Ada' });
  const doubled = derived(count, (c) => c * 2);
</script>

<window title="Stores">
  <box orientation="vertical" spacing="4">
    <label>{$count} x2={$doubled} {$user.name}</label>
    <button onclicked={() => $count++}>Inc</button>
    <button onclicked={() => ($count *= 3)}>Triple</button>
    <button onclicked={() => ($user.name = $user.name + '!')}>Shout</button>
    <button onclicked={() => ($count = 0)}>Reset</button>
  </box>
</window>
