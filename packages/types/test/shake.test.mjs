// Q9: import-driven tree shaking. A bundle that imports a few classes contains those classes and
// the classes their code imports, nothing else; and everything it contains is inside the signature
// closure the manifest computes. Dev builds stay fully reflective (no registry, nothing at import time).
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { build } from 'vite';
import { typesAliases } from '../vite.js';

const here = path.dirname(fileURLToPath(import.meta.url));
const manifest = JSON.parse(fs.readFileSync(path.join(here, '../gtk/manifest.json'), 'utf8'));
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
  const dir = fs.mkdtempSync(path.join(fs.realpathSync(process.env.TMPDIR ?? '/tmp'), 'svl-shake-'));
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

test('importing ListStore keeps ListStore, its parent and interface, and nothing else', async () => {
  const code = await bundle("import { ListStore } from 'svelters:gtk/gio'; globalThis.x = new ListStore({});");
  const kept = natives(code).sort();
  assert.ok(kept.includes('Gio.ListStore'));
  assert.ok(kept.length <= 3, `kept ${kept.join(', ')}`);
  assert.ok(!kept.includes('Gtk.TextBuffer'));
  assert.ok(!/class TextBuffer/.test(code));
});

test('an unused import drops every class of the namespace', async () => {
  const code = await bundle("import { File } from 'svelters:gtk/gio'; globalThis.x = 1; void File;".replace('void File;', ''));
  assert.deepEqual(natives(code), []);
});

test('a bundle holds only classes inside the signature closure of what it imports', async () => {
  const code = await bundle(
    "import { File } from 'svelters:gtk/gio'; import { TextBuffer } from 'svelters:gtk/gtk'; import { FontDescription } from 'svelters:gtk/pango'; globalThis.x = [File, TextBuffer, FontDescription];",
  );
  const kept = new Set(natives(code));
  const allowed = closureOf(['Gio.File', 'Gtk.TextBuffer', 'Pango.FontDescription']);
  for (const n of kept) assert.ok(allowed.has(n), `${n} is in the bundle but not in the signature closure`);
  for (const n of ['Gio.File', 'Gtk.TextBuffer', 'Pango.FontDescription']) assert.ok(kept.has(n), n);
  // far less than the namespace: Gtk alone has more than a hundred classes
  assert.ok(kept.size < 120, `${kept.size} classes kept`);
});

test('no generated file runs anything at import time', () => {
  const dir = path.join(here, '../gtk');
  let n = 0;
  for (const ns of fs.readdirSync(dir, { withFileTypes: true }).filter((d) => d.isDirectory())) {
    for (const f of fs.readdirSync(path.join(dir, ns.name)).filter((x) => x.endsWith('.js') && x !== 'index.js')) {
      const src = fs.readFileSync(path.join(dir, ns.name, f), 'utf8');
      const top = src.split('\n').filter((l) => l && !l.startsWith(' ') && !l.startsWith('}') && !l.startsWith('//'));
      for (const l of top) assert.ok(/^(import \{|export class )/.test(l), `${ns.name}/${f}: top-level statement ${l}`);
      n++;
    }
  }
  assert.ok(n > 100, `${n} class files checked`);
  const pkg = JSON.parse(fs.readFileSync(path.join(here, '../package.json'), 'utf8'));
  assert.equal(pkg.sideEffects, false);
});
