// Q9 for the WinUI modules (N4): a bundle that imports a few `svelters:winui/<namespace>` classes contains those
// classes and the classes their code imports, all inside the signature closure of packages/types/winui/manifest.json.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { build } from 'vite';
import { typesAliases } from '../vite.js';

const here = path.dirname(fileURLToPath(import.meta.url));
const manifest = JSON.parse(fs.readFileSync(path.join(here, '../winui/manifest.json'), 'utf8'));
const byNative = {};
for (const m of Object.values(manifest.modules)) for (const e of Object.values(m.exports)) byNative[e.native] = e;

function closureOf(natives) {
  const seen = new Set();
  const todo = [...natives];
  while (todo.length) {
    const n = todo.pop();
    if (seen.has(n)) continue;
    seen.add(n);
    const e = byNative[n];
    if (!e) continue;
    if (e.extends) todo.push(e.extends);
    todo.push(...e.implements, ...e.signature, ...e.imports);
  }
  return seen;
}

async function bundle(source) {
  const dir = fs.mkdtempSync(path.join(fs.realpathSync(process.env.TMPDIR ?? '/tmp'), 'svl-shake-winui-'));
  const entry = path.join(dir, 'main.js');
  fs.writeFileSync(entry, source);
  const out = await build({
    root: dir,
    configFile: false,
    logLevel: 'silent',
    resolve: { alias: typesAliases() },
    build: { write: false, minify: false, target: 'es2022', lib: { entry, formats: ['es'], fileName: 'app' } },
  });
  const chunks = (Array.isArray(out) ? out : [out]).flatMap((o) => o.output);
  fs.rmSync(dir, { recursive: true, force: true });
  return chunks.map((c) => c.code ?? '').join('\n');
}

const natives = (code) => [...code.matchAll(/static native = ["']([^"']+)["']/g)].map((m) => m[1]);

test('importing PropertySet keeps PropertySet and what its signatures wrap only', async () => {
  const code = await bundle("import { PropertySet } from 'svelters:winui/windows.foundation.collections'; globalThis.x = PropertySet;");
  const kept = natives(code);
  assert.ok(kept.includes('Windows.Foundation.Collections.PropertySet'));
  const allowed = closureOf(['Windows.Foundation.Collections.PropertySet']);
  for (const n of kept) assert.ok(allowed.has(n), `${n} is in the bundle but not in the signature closure`);
});

test('an unused import drops every class of the module', async () => {
  const code = await bundle("import { Uri } from 'svelters:winui/windows.foundation'; globalThis.x = 1;");
  assert.deepEqual(natives(code), []);
});

test('a WinUI bundle holds only classes inside the signature closure of what it imports', async () => {
  const roots = ['Windows.Foundation.Uri', 'Windows.Storage.StorageFolder', 'Microsoft.UI.Xaml.Media.SolidColorBrush'];
  const code = await bundle(
    "import { Uri } from 'svelters:winui/windows.foundation'; import { StorageFolder } from 'svelters:winui/windows.storage';"
      + " import { SolidColorBrush } from 'svelters:winui/microsoft.ui.xaml.media'; globalThis.x = [Uri, StorageFolder, SolidColorBrush];",
  );
  const kept = new Set(natives(code));
  const allowed = closureOf(roots);
  for (const n of kept) assert.ok(allowed.has(n), `${n} is in the bundle but not in the signature closure`);
  for (const n of roots) assert.ok(kept.has(n), n);
  assert.ok(kept.size < Object.keys(byNative).length, `${kept.size} classes kept`);
});

test('no generated WinUI file runs anything at import time', () => {
  const dir = path.join(here, '../winui');
  let n = 0;
  for (const ns of fs.readdirSync(dir, { withFileTypes: true }).filter((d) => d.isDirectory())) {
    for (const f of fs.readdirSync(path.join(dir, ns.name)).filter((x) => x.endsWith('.js') && x !== 'index.js')) {
      const src = fs.readFileSync(path.join(dir, ns.name, f), 'utf8');
      const top = src.split('\n').filter((l) => l && !l.startsWith(' ') && !l.startsWith('}') && !l.startsWith('//'));
      for (const l of top) assert.ok(/^(import \{|export class )/.test(l), `${ns.name}/${f}: top-level statement ${l}`);
      n++;
    }
  }
  assert.ok(n > 50, `${n} class files checked`);
});

test('every WinUI module imports and its classes have native names', async () => {
  for (const m of Object.values(manifest.modules)) {
    const mod = await import(path.join(here, `../winui/${m.dir}/index.js`));
    for (const [name, C] of Object.entries(mod)) assert.equal(C.native.split('.').pop(), name);
  }
});

test('async arguments: trailing { signal } is split off and the rest padded to the widest overload', async () => {
  const { splitOptions, isPropsFor } = await import('../winui/_rt.js');
  const signal = { aborted: false, addEventListener() {} };
  assert.deepEqual(splitOptions(['a'], [1, 2]), [['a', null], undefined]);
  const [a, o] = splitOptions(['a', { signal }], [1, 2]);
  assert.deepEqual(a, ['a', null]);
  assert.equal(o.signal, signal);
  assert.deepEqual(splitOptions(['a', 'b'], [1, 2]), [['a', 'b'], undefined]);
  assert.equal(isPropsFor([{ opacity: 0.5 }], { opacity: ['opacity'], color: ['color'] }), true);
  assert.equal(isPropsFor([{ a: 255, r: 0, g: 0, b: 0 }], { opacity: ['opacity'], color: ['color'] }), false);
});
