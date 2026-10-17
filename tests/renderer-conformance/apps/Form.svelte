<script>
  import Entry from './Entry.svelte';
  let name = $state('Ann');
  export function getName() { return name; }
  export function setName(v) { name = v; }
</script>
<box>
  <Entry bind:value={name} placeholder="Name" />
  <label text={'Hello ' + name} />
</box>
