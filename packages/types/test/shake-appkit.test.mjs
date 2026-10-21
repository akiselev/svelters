// Q9 for the AppKit modules (N4): a bundle that imports a few `svelters:appkit/<fw>` classes contains those
// classes and the classes their code imports, all inside the signature closure of packages/types/appkit/manifest.json.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { build } from 'vite';
import { typesAliases } from '../vite.js';

const here = path.dirname(fileURLToPath(import.meta.url));
const manifest = JSON.parse(fs.readFileSync(path.join(here, '../appkit/manifest.json'), 'utf8'));
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
    todo.push(...e.implements, ...e.signature);
  }
  return seen;
}

async function bundle(source) {
  const dir = fs.mkdtempSync(path.join(fs.realpathSync(process.env.TMPDIR ?? '/tmp'), 'svl-shake-appkit-'));
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

test('an unused import drops every class and enum of the module', async () => {
  const code = await bundle("import { NSColor, NSColorType } from 'svelters:appkit/appkit'; import { NSURL, NSUUID } from 'svelters:appkit/foundation'; globalThis.x = 1;");
  assert.deepEqual(natives(code), []);
  assert.doesNotMatch(code, /ComponentBased/);
});

test('an AppKit bundle holds only classes inside the signature closure of what it imports', async () => {
  const roots = ['NSPasteboard', 'NSURL', 'NSUndoManager'];
  const code = await bundle(
    "import { NSPasteboard } from 'svelters:appkit/appkit'; import { NSURL, NSUndoManager } from 'svelters:appkit/foundation';" +
      " globalThis.x = [NSPasteboard.generalPasteboard(), new NSURL('https://example.com'), new NSUndoManager()];",
  );
  const got = natives(code);
  for (const r of roots) assert.ok(got.includes(r), `${r} is in the bundle`);
  const closure = closureOf(roots);
  for (const n of got) assert.ok(closure.has(n), `${n} is in the signature closure of ${roots.join(', ')}`);
});

test('every generated module file loads and every class names its native type', async () => {
  for (const [mod, m] of Object.entries(manifest.modules)) {
    const ns = await import(path.join(here, '..', 'appkit', m.dir, 'index.js'));
    for (const [name, e] of Object.entries(m.exports)) {
      assert.equal(ns[name]?.native, e.native, `${mod} exports ${name}`);
    }
  }
});
