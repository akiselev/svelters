// svelte-check compiles with the same renderer option the build uses; without it every
// element `bind:` other than the DOM ones is a compile error.
export default {
  compilerOptions: { experimental: { customRenderer: '@svelters/renderer' } },
};
