<script>
  import Child from './Child.svelte';
  function h() {}
</script>

<box>
  <button on:clicked|preventDefault|once={h} />
  <button on:clicked|capture={h} />
  <button on:clicked|stopPropagation />
  <Child on:accepted|once={h} />
</box>
