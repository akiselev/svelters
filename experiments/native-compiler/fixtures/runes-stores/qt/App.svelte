<!-- Qt-vocabulary copy of ../App.svelte (the qt compile target). -->
<!-- Store auto-subscriptions: $store reads, assignments, compound assignments, updates and
     member mutations, plus a derived store. -->
<script>
  import { writable, derived } from 'svelte/store';
  const count = writable(1);
  const user = writable({ name: 'Ada' });
  const doubled = derived(count, (c) => c * 2);
</script>

<main-window window-title="Stores">
  <box-layout direction="top-to-bottom" spacing="4">
    <label>{$count} x2={$doubled} {$user.name}</label>
    <push-button onclicked={() => $count++}>Inc</push-button>
    <push-button onclicked={() => ($count *= 3)}>Triple</push-button>
    <push-button onclicked={() => ($user.name = $user.name + '!')}>Shout</push-button>
    <push-button onclicked={() => ($count = 0)}>Reset</push-button>
  </box-layout>
</main-window>
