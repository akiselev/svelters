let count = $state(0);
export const d = $derived(count * 2);
export { count };
count = 1;
const fixed = $state(1);
fixed = 2;
function f() {
  return $state(1);
}
$effect(() => {});
const x = $props();
