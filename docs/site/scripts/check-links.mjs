// Broken-link checker for the built site (dist/). Checks every internal href/src in every HTML page:
// the target file must exist and a #fragment must name an id (or <a name>) in the target page.
// External links (http/https/mailto) are not fetched: the build and CI stay offline.
// Usage: node scripts/check-links.mjs [dist-dir]
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { BASE } from '../site.config.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const dist = path.resolve(process.argv[2] ?? path.join(here, '../dist'));
if (!fs.existsSync(dist)) {
  console.error(`check-links: ${dist} does not exist; build first`);
  process.exit(2);
}

function* walk(dir) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) yield* walk(p);
    else yield p;
  }
}

const htmlFiles = [...walk(dist)].filter((f) => f.endsWith('.html'));
const idCache = new Map();
function idsOf(file) {
  if (!idCache.has(file)) {
    const html = fs.readFileSync(file, 'utf8');
    const ids = new Set();
    for (const m of html.matchAll(/\s(?:id|name)="([^"]+)"/g)) ids.add(m[1]);
    idCache.set(file, ids);
  }
  return idCache.get(file);
}

function resolveFile(urlPath) {
  const p = path.join(dist, decodeURIComponent(urlPath));
  if (fs.existsSync(p) && fs.statSync(p).isFile()) return p;
  for (const c of [path.join(p, 'index.html'), p + '.html']) if (fs.existsSync(c)) return c;
  return null;
}

const base = BASE.replace(/\/$/, '');
const errors = [];
let checked = 0;
for (const file of htmlFiles) {
  const html = fs.readFileSync(file, 'utf8').replace(/<script[\s\S]*?<\/script>/g, '');
  const pageUrl = '/' + path.relative(dist, file).replace(/index\.html$/, '').replace(/\\/g, '/');
  const refs = [...html.matchAll(/\s(?:href|src)="([^"]*)"/g)].map((m) => m[1].replace(/&amp;/g, '&'));
  for (const raw of refs) {
    if (!raw || /^(?:[a-z][a-z0-9+.-]*:|\/\/)/i.test(raw)) continue;
    let [target, frag] = raw.split('#');
    target = target.split('?')[0];
    let urlPath;
    if (target === '') urlPath = pageUrl;
    else if (target.startsWith('/')) {
      if (base && !target.startsWith(base + '/') && target !== base) { errors.push(`${pageUrl}: link outside base: ${raw}`); continue; }
      urlPath = target.slice(base.length) || '/';
    } else urlPath = new URL(target, 'http://x' + pageUrl).pathname;
    checked++;
    const f = resolveFile(urlPath);
    if (!f) {
      errors.push(`${pageUrl}: missing target ${raw}`);
      continue;
    }
    if (frag && f.endsWith('.html') && !idsOf(f).has(decodeURIComponent(frag))) errors.push(`${pageUrl}: missing anchor ${raw}`);
  }
}

if (errors.length) {
  const uniq = [...new Set(errors)];
  console.error(`check-links: ${uniq.length} broken links (of ${checked} internal references in ${htmlFiles.length} pages)`);
  for (const e of uniq.slice(0, 60)) console.error('  ' + e);
  if (uniq.length > 60) console.error(`  ... and ${uniq.length - 60} more`);
  process.exit(1);
}
console.log(`check-links: ${checked} internal references in ${htmlFiles.length} pages, all resolve`);
