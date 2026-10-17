<script>
  let items = $state([{ id: 1, name: 'a' }, { id: 2, name: 'b' }, { id: 3, name: 'c' }]);
  export function reverse() { items.reverse(); }
  export function insert() { items.splice(1, 0, { id: 4, name: 'd' }); }
  export function removeFirst() { items.shift(); }
  export function rename() { items[0].name = 'Z'; }
</script>
<list>
  {#each items as item (item.id)}
    <row key={item.id}>{item.name}</row>
  {/each}
</list>
