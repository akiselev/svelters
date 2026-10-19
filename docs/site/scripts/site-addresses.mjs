// Every element/type page and every member anchor the site renders, as JSON, for the docs-pack
// consistency test (crates/svelters-cli/tests/site_consistency.rs): the CLI must resolve each of
// them to the same page and anchor, so `svelters docs` and the site cannot drift.
// Usage: node scripts/site-addresses.mjs [--ir <dir>]
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderSite } from './lib/render.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const i = process.argv.indexOf('--ir');
const irDir = i > 0 ? path.resolve(process.argv[i + 1]) : path.resolve(here, '../.generated/ir');
const irs = fs
  .readdirSync(irDir)
  .filter((f) => f.endsWith('.json'))
  .sort()
  .map((f) => JSON.parse(fs.readFileSync(path.join(irDir, f), 'utf8')));
const { pages, model, routes } = renderSite(irs, { base: '/' });

const unesc = (s) => s.replace(/\\\|/g, '|');
const out = {};
for (const ir of model.irs) {
  const tk = ir.toolkit.id;
  const list = [];
  for (const e of ir.entities) {
    const route = routes[e.id];
    const md = pages.get(route.slice(1, -1) + '.md');
    const own = [];
    const inherited = [];
    // own property/event rows: | [`name`](#property-x) …
    for (const m of md.matchAll(/^\| \[`([^`]+)`\]\(#(property|event)-([^)]+)\)/gm)) own.push({ kind: m[2], name: unesc(m[1]), anchor: `${m[2]}-${m[3]}` });
    // methods, constructors and enum values: | <a id="method-x"></a>`name`
    for (const m of md.matchAll(/^\| <a id="((method|constructor|value)-[^"]+)"><\/a>`([^`]+)`/gm)) own.push({ kind: m[2], name: unesc(m[3]), anchor: m[1] });
    // inherited rows and lists link to the declaring page: [`name`](/api/tk/x/#kind-y)
    for (const m of md.matchAll(/\[`([^`]+)`\]\((\/api\/[^)#]+)#(property|event|method)-([^)]+)\)/g)) {
      if (m[2] === route) continue; // a link to this page (dangerous list) is an own member
      inherited.push({ kind: m[3], name: unesc(m[1]), route: m[2], anchor: `${m[3]}-${m[4]}` });
    }
    list.push({ id: e.id, route, tags: e.tags, own, inherited });
  }
  out[tk] = list;
}
process.stdout.write(JSON.stringify(out) + '\n');
