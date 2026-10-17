<script>
  import { writable } from 'svelte/store';
  const count = writable(0);
  const user = writable({ name: 'Ada', tags: [] });
  let { theme } = $props();

  function bump() {
    $count++;
    ++$count;
    $count += 2;
    $count = $count * 2;
    $user.name = 'Grace';
    $user.tags.push('x');
    $user.tags[0] = 'y';
  }
</script>

<label label={`${$count} ${$user.name} ${$theme}`} />
<entry bind:text={$user.name} />
