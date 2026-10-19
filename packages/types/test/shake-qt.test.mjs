// Q9 for the Qt modules (N2): a bundle that imports a few `svelters:qt/<ns>` classes contains those classes
// and the classes their code imports, all inside the signature closure of packages/types/qt/manifest.json.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { build } from 'vite';
import { typesAliases } from '../vite.js';

const here = path.dirname(fileURLToPath(import.meta.url));
const manifest = JSON.parse(fs.readFileSync(path.join(here, '../qt/manifest.json'), 'utf8'));
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
  const dir = fs.mkdtempSync(path.join(fs.realpathSync(process.env.TMPDIR ?? '/tmp'), 'svl-shake-qt-'));
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

test('importing QTimer keeps QTimer and QObject only', async () => {
  const code = await bundle("import { QTimer } from 'svelters:qt/core'; globalThis.x = new QTimer({ interval: 5 });");
  assert.deepEqual(natives(code).sort(), ['QObject', 'QTimer']);
});

test('an unused import drops every class of the module', async () => {
  const code = await bundle("import { QUrl } from 'svelters:qt/core'; globalThis.x = 1;");
  assert.deepEqual(natives(code), []);
});

test('a Qt bundle holds only classes inside the signature closure of what it imports', async () => {
  const roots = ['QStringListModel', 'QTextDocument', 'QUrl'];
  const code = await bundle(
    "import { QStringListModel, QUrl } from 'svelters:qt/core'; import { QTextDocument } from 'svelters:qt/gui'; globalThis.x = [QStringListModel, QTextDocument, QUrl];",
  );
  const kept = new Set(natives(code));
  const allowed = closureOf(roots);
  for (const n of kept) assert.ok(allowed.has(n), `${n} is in the bundle but not in the signature closure`);
  for (const n of roots) assert.ok(kept.has(n), n);
  assert.ok(kept.size < Object.keys(byNative).length, `${kept.size} classes kept`);
});

test('no generated Qt file runs anything at import time', () => {
  const dir = path.join(here, '../qt');
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

test('every Qt class module imports and its classes have native names', async () => {
  for (const ns of ['core', 'gui', 'widgets']) {
    const mod = await import(path.join(here, `../qt/${ns}/index.js`));
    for (const [name, C] of Object.entries(mod)) assert.equal(C.native, name);
  }
});
