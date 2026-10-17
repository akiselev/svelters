// Generate the API reference pages from the docs IR (docs/site/.generated/ir/*.json).
// Usage: node scripts/gen-pages.mjs [--ir <dir>] [--out <content/docs dir>]
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderSite } from './lib/render.mjs';
import { BASE, REPO, REPO_BRANCH, PLANNED_TOOLKITS, TOOLKIT_ORDER } from '../site.config.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const site = path.resolve(here, '..');

function arg(name, dflt) {
  const i = process.argv.indexOf(name);
  return i > 0 ? path.resolve(process.argv[i + 1]) : dflt;
}

const irDir = arg('--ir', path.join(site, '.generated/ir'));
const outDir = arg('--out', path.join(site, 'src/content/docs'));
const publicDir = arg('--public', path.join(site, 'public'));
const shotDir = path.join(site, '.generated/screenshots');

if (!fs.existsSync(irDir) || !fs.readdirSync(irDir).some((f) => f.endsWith('.json'))) {
  console.error(`no IR in ${irDir}; run: python3 tools/docs/build_ir.py`);
  process.exit(1);
}
const irs = fs
  .readdirSync(irDir)
  .filter((f) => f.endsWith('.json'))
  .sort()
  .map((f) => JSON.parse(fs.readFileSync(path.join(irDir, f), 'utf8')));

const screens = new Set();
const manifest = path.join(shotDir, 'manifest.json');
if (fs.existsSync(manifest)) for (const k of JSON.parse(fs.readFileSync(manifest, 'utf8')).keys ?? []) screens.add(k);

const { pages, routes } = renderSite(irs, { base: BASE, repo: REPO, branch: REPO_BRANCH, planned: PLANNED_TOOLKITS, order: TOOLKIT_ORDER, screens });

function writeIfChanged(file, text) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  if (fs.existsSync(file) && fs.readFileSync(file, 'utf8') === text) return false;
  fs.writeFileSync(file, text);
  return true;
}

// the generated api tree is replaced wholesale so removed kinds disappear
fs.rmSync(path.join(outDir, 'api'), { recursive: true, force: true });
let changed = 0;
for (const [rel, text] of pages) changed += writeIfChanged(path.join(outDir, rel), text) ? 1 : 0;

writeIfChanged(path.join(publicDir, 'toolkits.json'), JSON.stringify(irs.map((ir) => ({ id: ir.toolkit.id, name: ir.toolkit.name.replace(/ \(.*\)$/, ''), version: ir.toolkit.version })).sort((a, b) => TOOLKIT_ORDER.indexOf(a.id) - TOOLKIT_ORDER.indexOf(b.id))) + '\n');
writeIfChanged(path.join(publicDir, 'routes.json'), JSON.stringify(routes) + '\n');
fs.rmSync(path.join(publicDir, 'screenshots'), { recursive: true, force: true });
for (const k of screens) {
  const src = path.join(shotDir, k + '.png');
  if (fs.existsSync(src)) {
    fs.mkdirSync(path.dirname(path.join(publicDir, 'screenshots', k)), { recursive: true });
    fs.copyFileSync(src, path.join(publicDir, 'screenshots', k + '.png'));
  }
}
console.log(`docs pages: ${pages.size} generated (${changed} written), ${screens.size} screenshots`);
