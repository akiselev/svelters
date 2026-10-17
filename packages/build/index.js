import { svelte } from '@sveltejs/vite-plugin-svelte';

/**
 * Vite config for one fixture/app. Output: dist/app.js, a single self-contained ES module
 * (Svelte runtime + @svelters/dom shim + app) with no imports and no Node built-ins.
 */
export function svelteConfig(root) {
  return {
    root,
    resolve: { conditions: ['browser'] },
    plugins: [
      svelte({
        compilerOptions: { fragments: 'tree' },
        onwarn(warning, handler) {
          if (warning.code.startsWith('a11y_')) return;
          handler(warning);
        },
      }),
    ],
    build: {
      target: 'es2022',
      minify: false,
      sourcemap: true,
      cssCodeSplit: false,
      modulePreload: false,
      emptyOutDir: true,
      lib: { entry: 'src/main.js', formats: ['es'], fileName: () => 'app.js' },
    },
  };
}
