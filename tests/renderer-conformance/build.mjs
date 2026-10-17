// Bundle an entry plus its .svelte imports into one ESM file with the vendored Svelte fork.
// usage: node build.mjs <entry.mjs> <out.mjs> [dev] [hmr]
// The components are compiled with experimental.customRenderer -> ./renderer.mjs (the test renderer).
import path from 'node:path';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';
import { compile } from 'svelte/compiler';

const here = path.dirname(fileURLToPath(import.meta.url));

export async function bundle(entry, out, { dev = false, hmr = false } = {}) {
  const warnings = [];
  const sveltePlugin = {
    name: 'svelte',
    setup(b) {
      b.onLoad({ filter: /\.svelte$/ }, (args) => {
        const src = fs.readFileSync(args.path, 'utf8');
        let r;
        try {
          r = compile(src, {
            generate: 'client',
            dev,
            hmr,
            filename: path.basename(args.path),
            experimental: { customRenderer: path.join(here, 'renderer.mjs') },
          });
        } catch (e) {
          return { errors: [{ text: `${e.code ?? ''} ${e.message}` }] };
        }
        // No Vite here: route import.meta.hot to a host-provided registry keyed by file name.
        if (hmr) r.js.code = r.js.code.replaceAll('import.meta.hot', `globalThis.__hot?.[${JSON.stringify(path.basename(args.path))}]`);
        for (const w of r.warnings) warnings.push(`${path.basename(args.path)}: ${w.code}`);
        return { contents: r.js.code, loader: 'js', resolveDir: path.dirname(args.path) };
      });
    },
  };
  await build({
    entryPoints: [entry],
    outfile: out,
    bundle: true,
    format: 'esm',
    platform: 'neutral',
    mainFields: ['module', 'main'],
    conditions: ['custom-renderer', 'browser', dev ? 'development' : 'production'],
    define: { 'process.env.NODE_ENV': dev ? '"development"' : '"production"' },
    alias: { '#renderer': path.join(here, 'renderer.mjs') },
    plugins: [sveltePlugin],
    logLevel: 'error',
  });
  return warnings;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [entry, out, ...flags] = process.argv.slice(2);
  const w = await bundle(entry, out, { dev: flags.includes('dev'), hmr: flags.includes('hmr') });
  if (w.length) console.error('compile warnings:', w.join(', '));
}
