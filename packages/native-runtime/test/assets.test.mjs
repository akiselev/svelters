// AS1 (plans/research/ASSETS.md §1.4): the Vite plugin gives an asset import the same module as the
// Rust loader (the `Asset` handle of `asset://app/<logical path>`), and `svelters:assets`' handle
// converts to its ref everywhere a string is expected.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { sveltersAssets, ASSETS_ID } from '../vite.js';

function scratch() {
  const d = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'svelters-as1-')));
  fs.mkdirSync(path.join(d, 'src'));
  fs.writeFileSync(path.join(d, 'src/icon.svg'), '<svg/>');
  const pkg = path.join(d, 'node_modules/@acme/icons');
  fs.mkdirSync(pkg, { recursive: true });
  fs.writeFileSync(path.join(pkg, 'package.json'), '{"name":"@acme/icons"}');
  fs.writeFileSync(path.join(pkg, 'gear.svg'), '<svg/>');
  return d;
}

function ctx(root) {
  const emitted = [];
  return {
    emitted,
    async resolve(spec, importer) {
      const p = spec.startsWith('@acme/') ? path.join(root, 'node_modules', spec) : path.resolve(path.dirname(importer), spec);
      return fs.existsSync(p) ? { id: p } : null;
    },
    addWatchFile() {},
    emitFile(f) { emitted.push(f); },
  };
}

test('asset imports resolve to the same ref module as the Rust loader, and build copies them', async () => {
  const root = scratch();
  const plugin = sveltersAssets(root);
  const c = ctx(root);
  const importer = path.join(root, 'src/App.svelte');
  assert.deepEqual(await plugin.resolveId.call(c, ASSETS_ID, importer, {}), { id: ASSETS_ID, external: true });
  const id = await plugin.resolveId.call(c, './icon.svg', importer, {});
  assert.equal(await plugin.resolveId.call(c, './icon.svg?url', importer, {}), id);
  assert.equal(plugin.load.call(c, id), `import { asset } from 'svelters:assets';\nexport default asset("asset://app/src/icon.svg");\n`);
  const bytes = await plugin.resolveId.call(c, './icon.svg?bytes', importer, {});
  assert.match(plugin.load.call(c, bytes), /export default bytes\("asset:\/\/app\/src\/icon.svg"\)/);
  const dep = await plugin.resolveId.call(c, '@acme/icons/gear.svg', importer, {});
  assert.match(plugin.load.call(c, dep), /asset\("asset:\/\/app\/node_modules\/@acme\/icons\/gear.svg"\)/);
  // ?raw, ?inline and code stay Vite's
  assert.equal(await plugin.resolveId.call(c, './icon.svg?raw', importer, {}), null);
  assert.equal(await plugin.resolveId.call(c, './icon.svg?inline', importer, {}), null);
  assert.equal(await plugin.resolveId.call(c, './App.svelte', importer, {}), null);
  plugin.generateBundle.call(c);
  assert.deepEqual(c.emitted.map((f) => f.fileName).sort(), ['assets/node_modules/@acme/icons/gear.svg', 'assets/src/icon.svg']);
});

test('an Asset handle is one per ref and converts to its ref', async () => {
  const calls = [];
  globalThis.__svelters_host = {
    assets: (op, ref) => (calls.push([op, ref]), op === 'text' ? 'hi' : '{"mime":"image/svg+xml","len":6,"hash":"0","version":0}'),
    assetBytes: () => new Uint8Array([1, 2]).buffer,
  };
  const src = fs.readFileSync(new URL('../../../crates/svelters-modules/src/assets.js', import.meta.url), 'utf8');
  const m = await import(`data:text/javascript,${encodeURIComponent(src)}`);
  const a = m.asset('asset://app/src/icon.svg');
  assert.equal(a, m.asset('src/icon.svg'));
  assert.equal(String(a), 'asset://app/src/icon.svg');
  assert.equal(`${a}`, 'asset://app/src/icon.svg');
  assert.equal(JSON.stringify({ a }), '{"a":"asset://app/src/icon.svg"}');
  assert.deepEqual([a.root, a.path, a.size, a.mime, a.text()], ['app', 'src/icon.svg', 6, 'image/svg+xml', 'hi']);
  assert.deepEqual([...a.bytes()], [1, 2]);
  assert.equal(a.__svelters_asset, 'asset://app/src/icon.svg');
  assert.ok(!Object.keys(a).includes('__svelters_asset'));
  let seen = 0;
  const off = a.onChange((x) => { assert.equal(x, a); seen++; });
  globalThis.__svelters_assets_changed(['asset://app/src/icon.svg', 'asset://app/other.png']);
  off();
  globalThis.__svelters_assets_changed(['asset://app/src/icon.svg']);
  assert.equal(seen, 1);
  assert.equal(globalThis.__svelters_asset('asset://app/src/icon.svg'), a);
  assert.throws(() => m.text(42), TypeError);
  delete globalThis.__svelters_host;
});
