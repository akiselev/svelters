// IR -> Markdown pages. This module consumes ONLY svelters-docs-ir/v0 documents (see
// tools/docs/ir/schema.json); it never reads metadata/, GIR or qdoc, and has no toolkit-specific code.
// Toolkit specifics arrive in `extensions.<toolkit>` and render through `extension_schemas`.

const MEMBER_ANCHOR = { property: 'property', event: 'event', method: 'method', constructor: 'constructor', value: 'value', vfunc: 'vfunc' };

export function slugify(s) {
  return s.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '');
}

const esc = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const cell = (s) => String(s ?? '').replace(/\|/g, '\\|').replace(/\s*\n\s*/g, ' ').trim();
const yaml = (s) => JSON.stringify(String(s));

export class Model {
  constructor(irs, opts = {}) {
    this.base = opts.base ?? '/';
    this.repo = opts.repo ?? 'https://example.invalid/repo';
    this.branch = opts.branch ?? 'master';
    this.planned = opts.planned ?? [];
    this.screens = opts.screens ?? new Set();
    this.order = opts.order ?? [];
    this.irs = [...irs].sort((a, b) => this.rank(a.toolkit.id) - this.rank(b.toolkit.id) || a.toolkit.id.localeCompare(b.toolkit.id));
    this.toolkits = new Map(this.irs.map((ir) => [ir.toolkit.id, ir]));
    this.byId = new Map();
    this.routes = new Map(); // entity id -> path ("/api/gtk/button/")
    this.tagIndex = new Map(); // tag -> Map(toolkit -> entity)
    for (const ir of this.irs) {
      const seen = new Set();
      for (const e of ir.entities) {
        e._tk = ir.toolkit.id;
        this.byId.set(e.id, e);
        let path;
        if (e.tags.length) {
          path = `/api/${ir.toolkit.id}/${e.tags[0]}/`;
        } else {
          let slug = slugify(e.native_name || e.name);
          while (seen.has(slug)) slug += '-x';
          seen.add(slug);
          path = `/api/${ir.toolkit.id}/types/${slug}/`;
        }
        this.routes.set(e.id, path);
        for (const t of e.tags) {
          if (!this.tagIndex.has(t)) this.tagIndex.set(t, new Map());
          this.tagIndex.get(t).set(ir.toolkit.id, e);
        }
      }
    }
  }

  rank(id) {
    const i = this.order.indexOf(id);
    return i < 0 ? 99 : i;
  }

  url(path) {
    return this.base.replace(/\/$/, '') + path;
  }

  link(id, frag = '') {
    const p = this.routes.get(id);
    return p ? this.url(p) + frag : null;
  }

  // Member anchor for an xref fragment like "property:label".
  fragFor(frag) {
    if (!frag) return '';
    const [kind, ...rest] = frag.split(':');
    const k = MEMBER_ANCHOR[kind];
    return k ? `#${k}-${slugify(rest.join(':'))}` : '';
  }
}

// ---------- text ----------

const ALLOWED_HTML = new Set(['kbd', 'em', 'strong', 'code', 'br', 'sub', 'sup', 'b', 'i']);

function mapOutsideCode(md, fn) {
  // Split at fenced blocks, then at inline code spans, applying fn to the prose parts only.
  const out = [];
  const lines = md.split('\n');
  let fence = false;
  let buf = [];
  const flush = () => {
    if (!buf.length) return;
    const prose = buf.join('\n');
    out.push(prose.split(/(`[^`\n]*`)/).map((p, i) => (i % 2 ? p : fn(p))).join(''));
    buf = [];
  };
  for (const line of lines) {
    if (/^\s*```/.test(line)) {
      flush();
      fence = !fence;
      out.push(line);
    } else if (fence) {
      out.push(line);
    } else {
      buf.push(line);
      if (buf.length >= 1) {
        // keep line granularity so fences are detected; join later per line
        flush();
      }
    }
  }
  flush();
  return out.join('\n');
}

export function renderText(model, block, { toolkit, level = 5 } = {}) {
  if (!block || !block.md) return '';
  const upstream = model.toolkits.get(toolkit)?.toolkit.upstream_docs ?? '';
  let md = block.md;
  // demote headings to at least `level`
  let min = 7;
  let fence = false;
  for (const l of md.split('\n')) {
    if (/^\s*```/.test(l)) fence = !fence;
    const m = !fence && l.match(/^(#{1,6})\s/);
    if (m) min = Math.min(min, m[1].length);
  }
  if (min < 7) {
    const shift = Math.max(0, level - min);
    fence = false;
    md = md
      .split('\n')
      .map((l) => {
        if (/^\s*```/.test(l)) fence = !fence;
        const m = !fence && l.match(/^(#{1,6})(\s.*)$/);
        return m ? '#'.repeat(Math.min(6, m[1].length + shift)) + m[2] : l;
      })
      .join('\n');
  }
  md = mapOutsideCode(md, (p) => {
    // images cannot be fetched offline: drop them
    p = p.replace(/!\[[^\]]*\]\([^)]*\)/g, '');
    // xref links
    p = p.replace(/\[([^\]]+)\]\(xref:([^)#\s]+)(#[^)\s]+)?\)/g, (_, text, id, frag) => {
      const href = model.link(id, model.fragFor((frag ?? '').slice(1)));
      return href ? `[${text}](${href})` : text;
    });
    // relative upstream links
    p = p.replace(/\]\((?!https?:|xref:|#|\/|mailto:)([^)\s]+)\)/g, (_, rel) => (upstream ? `](${upstream}${rel})` : ']()'));
    // neutralise HTML that is not on the allow list, and bare braces that confuse MDX-like processors
    p = p.replace(/<(\/?)([A-Za-z][A-Za-z0-9-]*)([^<>]*)>/g, (m, slash, name, rest) =>
      ALLOWED_HTML.has(name.toLowerCase()) ? `<${slash}${name.toLowerCase()}>` : `&lt;${slash}${name}${rest}&gt;`);
    p = p.replace(/<(?![A-Za-z/])/g, '&lt;');
    return p;
  });
  return md.trim();
}

function brief(model, block, toolkit) {
  if (!block) return '';
  const t = renderText(model, block, { toolkit });
  return t.split(/\n\s*\n/)[0].replace(/\s+/g, ' ');
}

function plain(s) {
  return s.replace(/\[([^\]]*)\]\([^)]*\)/g, '$1').replace(/[`*_]/g, '').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/<[^>]+>/g, '');
}

// ---------- small renderers ----------

const ORIGIN_TITLE = {
  gir: 'GObject-Introspection repository',
  runtime: 'PyGObject runtime probe',
  qdoc: 'qdoc index',
  metaobject: 'QMetaObject (PySide6)',
  typesystem: 'PySide typesystem',
  manual: 'Hand/agent annotation (metadata/overrides)',
  apiextractor: 'API extractor',
  derived: 'Derived by the docs generator',
};

export function badge(origin) {
  return `<span class="svl-badge svl-origin-${origin}" title="${esc(ORIGIN_TITLE[origin] ?? origin)}">${esc(origin)}</span>`;
}

const originBadges = (m) => (m.origins?.length ? m.origins : [m.origin]).map(badge).join(' ');

function typeMd(model, t) {
  if (!t) return '';
  const disp = '`' + String(t.display).replace(/`/g, "'") + '`';
  const href = t.ref ? model.link(t.ref) : null;
  const nul = t.nullable ? ' or `null`' : '';
  return (href ? `[${disp}](${href})` : disp) + nul;
}

function access(p) {
  if (p.access.construct_only) return 'construct-only';
  return p.access.read && p.access.write ? 'read / write' : p.access.write ? 'write' : 'read';
}

function availabilityMd(av) {
  if (!av) return '';
  const bits = [];
  if (av.since) bits.push(`since ${av.since}`);
  if (av.deprecated) bits.push(av.deprecated === true ? 'deprecated' : `deprecated since ${av.deprecated}`);
  return bits.join(', ');
}

function extFields(model, e, scope, rec) {
  const ir = model.toolkits.get(e._tk);
  const schema = ir.extension_schemas?.[e._tk];
  const ext = rec.extensions?.[e._tk];
  if (!schema || !ext) return [];
  const out = [];
  for (const f of schema.fields.filter((x) => x.scope === scope)) {
    const v = ext[f.key];
    if (v === undefined || v === null || v === '' || (Array.isArray(v) && !v.length)) continue;
    out.push({ ...f, value: v });
  }
  return out;
}

function extInline(fields) {
  return fields
    .map((f) => {
      const v = Array.isArray(f.value) ? f.value.map((x) => '`' + x + '`').join(', ') : f.format === 'code' ? '`' + f.value + '`' : String(f.value);
      return `<span class="svl-ext"><b>${esc(f.label)}:</b> ${v.replace(/\n/g, ' ')}</span>`;
    })
    .join(' ');
}

// ---------- inheritance ----------

function ancestors(model, e) {
  const out = [];
  const seen = new Set([e.id]);
  for (const id of [...e.bases, ...e.interfaces]) {
    const a = model.byId.get(id);
    if (a && !seen.has(a.id)) {
      seen.add(a.id);
      out.push(a);
    }
  }
  return out;
}

// ---------- entity page ----------

function memberAnchor(kind, name, used) {
  let a = `${kind}-${slugify(name)}`;
  let n = 1;
  while (used.has(a)) a = `${kind}-${slugify(name)}-${++n}`;
  used.add(a);
  return a;
}

// ---------- dangerous members (IR `dangerous`, tools/metadata/common/dangerous.py) ----------

const SEVERITY_LABEL = {
  'corrupts-tree': 'corrupts the widget tree',
  'owned-by-backend': 'owned by the backend',
  crashes: 'can crash',
  other: 'backend-owned state',
};
const dangerText = (s) => mapOutsideCode(String(s), esc);
const DANGER_MARK = ' <span class="svl-danger" title="Dangerous: the backend owns what this changes">⚠ dangerous</span>';
const dangerMark = (m) => (m?.dangerous ? DANGER_MARK : '');

// The warning callout on a dangerous member's details.
function dangerCallout(d, what) {
  const how = d.reachable === false
    ? `${what} is not possible from app code today; the backend owns this state, so do not expose it`
    : `${what} is reachable from app code (a native call or attribute), but the backend owns what it changes`;
  const o = [`:::danger[Dangerous: ${SEVERITY_LABEL[d.severity] ?? d.severity}]`, `${how}. ${dangerText(d.reason)}`];
  if (d.use_instead) o.push('', `**Use instead:** ${dangerText(d.use_instead)}`);
  o.push('', `<small>Declared in <code>${esc(d.source)}</code>. Nothing blocks it at run time yet.</small>`, ':::', '');
  return o;
}

// The per-kind list: own and inherited dangerous members, linking to their details.
function dangerSection(model, e, anc) {
  const KINDS = [['properties', 'property', 'property'], ['events', 'event', 'event'], ['methods', 'method', 'method']];
  const rows = [];
  for (const owner of [e, ...anc]) {
    for (const [field, kind, anchor] of KINDS) {
      for (const m of owner[field] ?? []) {
        if (!m.dangerous) continue;
        if (rows.some((r) => r.field === field && r.m.name === m.name)) continue; // overridden lower in the chain
        rows.push({ field, kind, anchor, m, owner });
      }
    }
  }
  if (!rows.length) return [];
  const o = ['## Dangerous members', '', `:::danger[${rows.length} dangerous member${rows.length === 1 ? '' : 's'}]`,
    'These are reachable from app code (through `call()`, `setProp` or an attribute) unless marked otherwise, but the backend owns what they change: using them can corrupt the widget tree svelters manages, fight its lifecycle or crash. Nothing blocks them at run time yet.', ':::', '',
    '| Member | Kind | Severity | Why | Use instead |', '| --- | --- | --- | --- | --- |'];
  for (const r of rows) {
    const here = r.owner === e;
    const href = `${here ? '' : model.link(r.owner.id)}#${r.anchor}-${slugify(r.m.name)}`;
    const label = r.field === 'properties' ? r.m.js_name : r.m.name;
    o.push(`| [\`${cell(label)}\`](${href})${here ? '' : ` <small>from ${esc(r.owner.name)}</small>`} | ${r.kind}${r.m.dangerous.reachable === false ? ' (not reachable today)' : ''} | ${esc(SEVERITY_LABEL[r.m.dangerous.severity] ?? r.m.dangerous.severity)} | ${cell(dangerText(r.m.dangerous.reason))} | ${cell(dangerText(r.m.dangerous.use_instead ?? '–'))} |`);
  }
  o.push('');
  return o;
}

function tierBadge(e) {
  return e.status.tier === 'annotated'
    ? '<span class="svl-tier svl-tier-annotated" title="Hand/agent-annotated, with generated verification tests">Annotated &amp; tested</span>'
    : '<span class="svl-tier svl-tier-raw" title="Ingested from upstream metadata only">Raw ingested</span>';
}

export function renderEntity(model, e) {
  const tk = e._tk;
  const ir = model.toolkits.get(tk);
  const tkName = ir.toolkit.name;
  const title = e.tags.length ? `<${e.tags[0]}>` : e.native_name;
  const used = new Set();
  const o = [];
  const desc = brief(model, e.summary, tk);
  const fm = ['---', `title: ${yaml(title + (e.tags.length ? ` (${e.native_name})` : ''))}`];
  fm.push(`description: ${yaml(plain(desc) || `${e.native_name} in ${tkName}`)}`);
  fm.push('sidebar:', '  hidden: true', 'pagefind: true');
  const af = e.status.annotation_files?.[0];
  const overridePath = `metadata/overrides/${tk}/${(e.status.metadata_file ?? '').split('/').pop()?.replace(/\.json$/, '') || e.name}.yaml`;
  const editUrl = af ? `${model.repo}/edit/${model.branch}/${af}` : `${model.repo}/new/${model.branch}/metadata/overrides/${tk}?filename=${overridePath.split('/').pop()}`;
  fm.push(`editUrl: ${yaml(editUrl)}`, '---', '');
  o.push(...fm);

  // header badges
  const head = [`<span class="svl-toolkit svl-toolkit-${tk}">${esc(tkName)}</span>`, tierBadge(e)];
  if (e.tags.length) head.push(`<code class="svl-tag">&lt;${esc(e.tags[0])}&gt;</code>`);
  head.push(`<span class="svl-kind">${esc(e.kind)}${e.abstract ? ' (abstract)' : ''}</span>`);
  const av = availabilityMd(e.availability);
  if (av) head.push(`<span class="svl-kind">${esc(av)}</span>`);
  if (e.lifetime && e.lifetime.class !== 'unknown') head.push(`<a class="svl-kind" href="${model.url('/concepts/ownership/')}#lifetime-classes">lifetime: ${esc(e.lifetime.class)}</a>`);
  o.push(`<div class="svl-head">${head.join(' ')}</div>`, '');

  if (e.status.tier !== 'annotated') {
    o.push(':::caution[Raw ingested kind]', e.status.note || 'This kind is ingested from upstream metadata and is not annotated or tested yet.', `You can promote it by adding an annotation: [create the override file](${editUrl}). See [the annotation workflow](${model.url('/guides/annotation-workflow/')}).`, ':::', '');
  } else {
    o.push(`:::tip[Annotated and tested]`, `${esc(e.status.test_note ?? 'Annotated facts have generated verification tests.')} [Edit this annotation](${editUrl}).`, ':::', '');
  }
  if (e.availability?.deprecated_note) o.push(':::caution[Deprecated]', renderText(model, { md: e.availability.deprecated_note }, { toolkit: tk, level: 5 }), ':::', '');

  if (e.summary) o.push(renderText(model, e.summary, { toolkit: tk }), '');
  o.push(`[${esc(e.upstream.label)}](${e.upstream.url})`, '');

  // support matrix
  if (e.tags.length) {
    const rows = [];
    const t = model.tagIndex.get(e.tags[0]);
    for (const [id, ir2] of model.toolkits) {
      const ent = t?.get(id);
      rows.push(`<td>${ent ? `<a href="${model.link(ent.id)}">${tierMark(ent)}</a>` : '<span class="svl-no">not available</span>'}</td>`);
    }
    for (const p of model.planned) rows.push(`<td><span class="svl-planned">${esc(p.status)}</span></td>`);
    const heads = [...model.toolkits.values()].map((x) => `<th>${esc(x.toolkit.name)}</th>`).concat(model.planned.map((p) => `<th>${esc(p.name)}</th>`));
    o.push('### Toolkit support', '', `<table class="svl-matrix"><thead><tr>${heads.join('')}</tr></thead><tbody><tr>${rows.join('')}</tr></tbody></table>`, '');
  }

  if (e.screenshot && model.screens.has(e.screenshot)) {
    o.push(`<figure class="svl-shot"><img src="${model.url('/screenshots/' + e.screenshot + '.png')}" alt="Screenshot of ${esc(title)} on ${esc(tkName)}" loading="lazy" width="320" /><figcaption>${esc(tkName)}, rendered headless under Xvfb (generated).</figcaption></figure>`, '');
  }

  if (e.examples?.length) {
    o.push('## Usage', '');
    for (const ex of e.examples) {
      o.push(`**${esc(ex.title)}**${ex.generated ? ' <span class="svl-kind" title="Generated from metadata; not compiled">generated, illustrative</span>' : ''}`, '', '```' + ex.lang, ex.code.trimEnd(), '```', '');
    }
  }

  // inheritance
  const anc = ancestors(model, e);
  if (e.bases.length || e.interfaces.length) {
    const chain = (ids, sep = ' → ') => ids.map((id) => {
      const a = model.byId.get(id);
      return a ? `[${a.name}](${model.link(id)})` : '`' + id.slice(id.indexOf(':') + 1) + '`';
    }).join(sep);
    o.push('## Inheritance', '');
    if (e.bases.length) o.push(`**Chain:** ${e.name} → ${chain(e.bases)}`, '');
    if (e.interfaces.length) o.push(`**Interfaces:** ${chain(e.interfaces, ', ')}`, '');
  }

  if (e.description) o.push('## Description', '', renderText(model, e.description, { toolkit: tk, level: 4 }), '');
  else if (e.summary && e.summary.truncated) o.push(`:::note[Documentation text]`, `Only a short brief is included here. Read the complete documentation upstream: [${e.upstream.label}](${e.upstream.url}).`, ':::', '');

  o.push(...dangerSection(model, e, anc));

  if (e.values.length) {
    o.push('## Values', '', '| Name | Value | Description |', '| --- | --- | --- |');
    for (const v of e.values) {
      const id = memberAnchor('value', v.name, used);
      o.push(`| <a id="${id}"></a>\`${cell(v.name)}\`${v.nick && v.nick !== v.name ? ` (\`${cell(v.nick)}\`)` : ''} | \`${cell(v.value)}\` | ${cell(plain(brief(model, v.doc, tk)))} |`);
    }
    o.push('');
  }

  // properties
  if (e.properties.length) {
    o.push('## Properties', '');
    const ownNames = new Set();
    o.push(...propSummary(model, e, e.properties, tk, used, true), '');
    for (const p of e.properties) ownNames.add(p.name);
    const inh = inheritedMembers(model, e, anc, 'properties', ownNames);
    if (inh.length) o.push(...inheritedBlock(model, inh, 'property', 'Inherited properties', tk), '');
  } else {
    const inh = inheritedMembers(model, e, anc, 'properties', new Set());
    if (inh.length) o.push('## Properties', '', 'This kind declares no properties of its own.', '', ...inheritedBlock(model, inh, 'property', 'Inherited properties', tk), '');
  }

  // events
  const ownEv = new Set(e.events.map((x) => x.name));
  const inhEv = inheritedMembers(model, e, anc, 'events', ownEv);
  if (e.events.length || inhEv.length) {
    o.push('## Events', '');
    if (e.events.length) o.push(...eventSection(model, e, tk, used), '');
    if (inhEv.length) o.push(...inheritedBlock(model, inhEv, 'event', 'Inherited events', tk), '');
  }

  // children
  if (e.children && (e.children.slots.length || e.children.candidates?.length || e.children.policy !== 'unknown')) {
    o.push('## Children and slots', '', `**Policy:** \`${e.children.policy}\`${e.children.default_slot ? `, default slot \`${e.children.default_slot}\`` : ''}`, '');
    if (e.children.slots.length) {
      o.push('| Slot | Cardinality | Accepts | Operations | Ownership effects |', '| --- | --- | --- | --- | --- |');
      for (const s of e.children.slots) {
        const ops = Object.entries(s.ops ?? {}).map(([k, v]) => `**${k}**: \`${cell(v)}\``).join('<br>');
        const own = Object.entries(s.ownership ?? {}).map(([k, v]) => `**${k}**: ${cell(v)}`).join('<br>');
        o.push(`| \`${cell(s.name)}\`${s.default ? ' (default)' : ''}${s.annotated ? ' ' + badge('manual') : ''} | ${s.cardinality}${s.order ? `, order \`${cell(s.order)}\`` : ''} | ${cell((s.accepts ?? []).join(', '))} | ${ops || '–'} | ${own || '–'}${s.on_remove ? `<br>on remove: \`${cell(s.on_remove)}\`` : ''} |`);
      }
      o.push('');
      const prov = e.children.slots.flatMap((s) => s.provenance ?? []);
      if (prov.length) o.push(`<small>Provenance: ${[...new Set(prov)].map(esc).join('; ')}</small>`, '');
    } else if (e.children.candidates?.length) {
      o.push('No slot is annotated yet. Candidates found by ingestion: ' + e.children.candidates.map((c) => '`' + c + '`').join(', ') + '.', '');
    }
  }

  // lifetime
  if (e.lifetime) {
    o.push('## Lifetime', '', `Class **${e.lifetime.class}** ${badge(e.lifetime.origin)}${e.lifetime.note ? ': ' + esc(e.lifetime.note) : ''} See [Ownership and lifetimes](${model.url('/concepts/ownership/')}).`, '');
  }

  // constructors + methods
  if (e.constructors.length) o.push('## Constructors', '', ...methodTable(model, e, e.constructors, 'constructor', tk, used), '');
  if (e.methods.length) {
    o.push('## Methods', '', ...methodTable(model, e, e.methods, 'method', tk, used), '');
    const own = new Set(e.methods.map((m) => m.name));
    const inh = inheritedMembers(model, e, anc, 'methods', own);
    if (inh.length) {
      o.push('### Inherited methods', '');
      for (const [a, list] of groupBy(inh, (x) => x.owner.id)) {
        const ow = list[0].owner;
        o.push(`<details><summary>From ${esc(ow.name)} (${list.length})</summary>`, '', list.map((x) => `[\`${cell(x.m.name)}\`](${model.link(ow.id)}#method-${slugify(x.m.name)})${x.m.dangerous ? ' ⚠' : ''}`).join(', '), '', '</details>', '');
      }
    }
  }

  // toolkit specifics panel
  const eFields = extFields(model, e, 'entity', e);
  if (eFields.length) {
    o.push(`## ${esc(ir.extension_schemas[tk].title)}`, '');
    for (const f of eFields) {
      if (f.format === 'markdown') o.push(`### ${esc(f.label)}`, '', renderText(model, { md: String(f.value) }, { toolkit: tk, level: 4 }), '');
      else if (f.format === 'list') o.push(`**${esc(f.label)}**`, '', ...f.value.map((x) => `- ${x}`), '');
      else o.push(`**${esc(f.label)}:** ${f.format === 'code' ? '`' + f.value + '`' : esc(f.value)}`, '');
    }
  }

  // cross-references
  const refs = [];
  if (e.abi_symbols?.length) refs.push(`ABI: ${e.abi_symbols.map((s) => `[\`${s}\`](${model.url('/concepts/abi/')}#${s})`).join(', ')}`);
  if (e.serdes) refs.push(`Serdes support: **${e.serdes.status}**${e.serdes.note ? ' (' + esc(e.serdes.note) + ')' : ''}`);
  if (e.see_also?.length) refs.push('See also: ' + e.see_also.map((id) => (model.link(id) ? `[${model.byId.get(id).name}](${model.link(id)})` : id)).join(', '));
  if (e.status.metadata_file) refs.push(`Metadata: \`${e.status.metadata_file}\``);
  o.push('## See also', '', ...refs.map((r) => `- ${r}`), '');

  const notices = ir.toolkit.notices ?? [];
  if (notices.length && (e.summary || e.description || e.properties.some((p) => p.doc))) {
    o.push(`<div class="svl-notice">${notices.map(esc).join(' ')} <a href="${ir.toolkit.doc_license.url}">${esc(ir.toolkit.doc_license.name)}</a>.</div>`, '');
  }
  return o.join('\n');
}

function tierMark(ent) {
  return ent.status.tier === 'annotated' ? '✔ annotated &amp; tested' : 'raw';
}

function groupBy(list, f) {
  const m = new Map();
  for (const x of list) {
    const k = f(x);
    if (!m.has(k)) m.set(k, []);
    m.get(k).push(x);
  }
  return m;
}

function inheritedMembers(model, e, anc, field, own) {
  const seen = new Set(own);
  const out = [];
  for (const a of anc) {
    for (const m of a[field]) {
      const key = m.name + (field === 'methods' ? '' : '');
      if (seen.has(key)) continue;
      seen.add(key);
      out.push({ owner: a, m });
    }
  }
  return out;
}

function inheritedBlock(model, list, kind, title, tk) {
  const o = [`### ${title}`, ''];
  for (const [, items] of groupBy(list, (x) => x.owner.id)) {
    const ow = items[0].owner;
    o.push(`<details><summary>From ${esc(ow.name)} (${items.length})</summary>`, '');
    if (kind === 'property') {
      o.push('| JS name | Type | Access | Description |', '| --- | --- | --- | --- |');
      for (const { m } of items) o.push(`| [\`${cell(m.js_name)}\`](${model.link(ow.id)}#property-${slugify(m.name)})${dangerMark(m)} | ${cell(typeMd(model, m.type))} | ${access(m)} | ${cell(plain(brief(model, m.doc, tk)))} |`);
    } else {
      o.push('| Event | Handler | Description |', '| --- | --- | --- |');
      for (const { m } of items) o.push(`| [\`${cell(m.name)}\`](${model.link(ow.id)}#event-${slugify(m.name)})${dangerMark(m)} | \`${cell(m.handler_prop)}\` | ${cell(plain(brief(model, m.doc, tk)))} |`);
    }
    o.push('', '</details>', '');
  }
  return o;
}

function propSummary(model, e, props, tk, used) {
  const o = ['| JS name | Type | Default | Range | Access | Notify / bind | Origin | Version |', '| --- | --- | --- | --- | --- | --- | --- | --- |'];
  const detail = [];
  for (const p of props) {
    const a = memberAnchor('property', p.name, used);
    const range = p.range && (p.range.min != null || p.range.max != null) ? `${fmtNum(p.range.min)} … ${fmtNum(p.range.max)}` : '–';
    const nb = p.notify ? `\`${cell(p.notify)}\`${p.bindable ? ' · bindable' : ''}` : '–';
    o.push(`| [\`${cell(p.js_name)}\`](#${a})${p.annotated ? ' ★' : ''}${dangerMark(p)} | ${cell(typeMd(model, p.type))} | ${p.default != null ? '`' + cell(p.default) + '`' : '–'} | ${range} | ${access(p)} | ${nb} | ${originBadges(p)} | ${cell(availabilityMd(p.availability)) || '–'} |`);
    const ex = extFields(model, e, 'property', p);
    const d = [`<a id="${a}"></a>`, `#### \`${p.js_name}\``, ''];
    d.push(`<div class="svl-meta">${originBadges(p)}${p.annotated ? ' <span class="svl-kind">annotated</span>' : ''} ${typeMd(model, p.type).startsWith('[') ? '' : ''}</div>`, '');
    const bits = [`Type ${typeMd(model, p.type)}`, `${access(p)}`];
    if (p.name !== p.js_name) bits.push(`native name \`${p.name}\``);
    if (p.default != null) bits.push(`default \`${p.default}\``);
    if (p.notify) bits.push(`notify \`${p.notify}\`${p.bindable ? ' (bindable: `bind:' + p.js_name + '`, planned in F1)' : ''}`);
    if (p.accessors && Object.keys(p.accessors).length) bits.push('accessors ' + Object.entries(p.accessors).map(([k, v]) => `${k} \`${v}\``).join(', '));
    const avp = availabilityMd(p.availability);
    if (avp) bits.push(avp);
    d.push(bits.join(' · '), '');
    if (p.dangerous) d.push(...dangerCallout(p.dangerous, 'Writing this property'));
    if (p.availability?.deprecated_note) d.push(`> Deprecated: ${plain(brief(model, { md: p.availability.deprecated_note }, tk))}`, '');
    if (p.doc) d.push(renderText(model, p.doc, { toolkit: tk, level: 5 }), '');
    if (ex.length) d.push(extInline(ex), '');
    detail.push(...d);
  }
  return [...o, '', '### Property details', '', ...detail];
}

function fmtNum(n) {
  if (n == null) return '';
  if (Math.abs(n) >= 1e300) return n < 0 ? '-∞' : '∞';
  return String(n);
}

function eventSection(model, e, tk, used) {
  const o = ['| Event | Handler attribute | Arguments | Returns | Sync / result | Phase | Origin |', '| --- | --- | --- | --- | --- | --- | --- |'];
  const detail = [];
  for (const ev of e.events) {
    const a = memberAnchor('event', ev.name, used);
    const args = ev.params.length ? ev.params.map((p) => `${p.name}: ${cell(typeMd(model, p.type))}`).join(', ') : '–';
    const handlers = [ev.handler_prop, ...(ev.handler_aliases ?? [])].map((h) => '`' + cell(h) + '`').join(' / ');
    const sync = ev.sync_required ? 'sync required' : 'async';
    o.push(`| [\`${cell(ev.name)}\`](#${a})${ev.annotated ? ' ★' : ''}${dangerMark(ev)} | ${handlers} | ${args} | ${cell(ev.returns?.display ?? 'void')} | ${sync}${ev.handler_result && ev.handler_result !== 'none' ? ', result `' + ev.handler_result + '`' : ''} | ${ev.phase ? '`' + cell(ev.phase) + '`' : '–'} | ${originBadges(ev)} |`);
    const d = [`<a id="${a}"></a>`, `#### \`${ev.name}\``, '', `<div class="svl-meta">${originBadges(ev)}${ev.annotated ? ' <span class="svl-kind">annotated</span>' : ''}</div>`, ''];
    d.push(`Handler: ${handlers} · arguments: ${args} · returns \`${ev.returns?.display ?? 'void'}\` · ${sync}${ev.phase ? ` · phase \`${ev.phase}\`` : ''}${ev.flags?.length ? ' · flags ' + ev.flags.map((f) => '`' + f + '`').join(', ') : ''}`, '');
    if (ev.dangerous) d.push(...dangerCallout(ev.dangerous, 'Using this event'));
    if (ev.doc) d.push(renderText(model, ev.doc, { toolkit: tk, level: 5 }), '');
    const ex = extFields(model, e, 'event', ev);
    if (ex.length) d.push(extInline(ex), '');
    detail.push(...d);
  }
  return [...o, '', '### Event details', '', ...detail];
}

function methodTable(model, e, list, kind, tk, used) {
  const o = ['| Name | Signature | Origin |', '| --- | --- | --- |'];
  const detail = [];
  for (const m of list) {
    const a = memberAnchor(kind, m.name, used);
    const flags = [m.static ? 'static' : '', m.virtual ? 'virtual' : '', m.access && m.access !== 'public' ? m.access : ''].filter(Boolean);
    o.push(`| <a id="${a}"></a>\`${cell(m.name)}\`${m.annotated ? ' ★' : ''}${dangerMark(m)} | \`${cell(m.signature)}\`${flags.length ? ' <small>' + flags.join(', ') + '</small>' : ''}${m.unsupported_reason ? ' <small>unsupported: ' + esc(m.unsupported_reason) + '</small>' : ''} | ${originBadges(m)} |`);
    if (m.doc || m.dangerous || extFields(model, e, 'method', m).length) {
      detail.push(`#### \`${m.name}\``, '', '`' + m.signature.replace(/`/g, "'") + '`', '');
      if (m.dangerous) detail.push(...dangerCallout(m.dangerous, `Calling \`${m.name}\``));
      if (m.doc) detail.push(renderText(model, m.doc, { toolkit: tk, level: 5 }), '');
      const ex = extFields(model, e, 'method', m);
      if (ex.length) detail.push(extInline(ex), '');
    }
  }
  return detail.length ? [...o, '', ...detail] : o;
}

// ---------- index pages ----------

function fm(title, description, extra = '') {
  return ['---', `title: ${yaml(title)}`, `description: ${yaml(description)}`, extra, '---', ''].filter((x) => x !== '').join('\n') + '\n';
}

function entityRow(model, e, tk) {
  return `| [${cell(e.tags.length ? '<' + e.tags[0] + '>' : e.name).replace(/</g, '\\<')}](${model.link(e.id)}) | ${esc(e.native_name)} | ${e.kind} | ${tierMark(e)} | ${cell(plain(brief(model, e.summary, tk))).slice(0, 140)} |`;
}

export function renderToolkitIndex(model, ir) {
  const tk = ir.toolkit.id;
  const ents = ir.entities;
  const els = ents.filter((e) => e.tags.length).sort((a, b) => a.tags[0].localeCompare(b.tags[0]));
  const o = [fm(`${ir.toolkit.name} elements`, `Element reference for ${ir.toolkit.name}`, 'sidebar:\n  label: Elements')];
  o.push(`<div class="svl-head"><span class="svl-toolkit svl-toolkit-${tk}">${esc(ir.toolkit.name)}</span> <span class="svl-kind">version ${esc(ir.toolkit.version)}</span> <span class="svl-kind">${esc(ir.toolkit.language)}</span></div>`, '');
  const c = ir.coverage;
  o.push(`${els.length} element tags (${c.annotated_kinds} annotated and tested, the rest raw ingested). ${c.kinds} kinds ingested in total; see [all types](${model.url(`/api/${tk}/types/`)}) and the [coverage dashboard](${model.url('/coverage/')}).`, '');
  o.push(':::note[Annotated vs raw]', 'Rows marked **✔ annotated & tested** have hand/agent-annotated facts that are verified by generated tests. **raw** rows are ingested from upstream metadata only; their child semantics, ownership and signal behaviour are not verified.', ':::', '');
  o.push('| Element | Native class | Kind | Status | Summary |', '| --- | --- | --- | --- | --- |');
  for (const e of els) o.push(entityRow(model, e, tk));
  o.push('');
  return o.join('\n');
}

export function renderTypesIndex(model, ir) {
  const tk = ir.toolkit.id;
  const types = ir.entities.filter((e) => !e.tags.length).sort((a, b) => a.native_name.localeCompare(b.native_name));
  const o = [fm(`${ir.toolkit.name} types`, `All non-element types ingested for ${ir.toolkit.name}`, 'sidebar:\n  label: Types')];
  o.push(`${types.length} types: base classes, interfaces, enumerations, flags and boxed values. Elements are listed on the [element index](${model.url(`/api/${tk}/`)}).`, '');
  o.push('| Type | Native name | Kind | Status | Summary |', '| --- | --- | --- | --- | --- |');
  for (const e of types) o.push(entityRow(model, e, tk));
  o.push('');
  return o.join('\n');
}

export function renderApiIndex(model) {
  const o = [fm('API reference', 'Generated element and type reference for every toolkit', 'sidebar:\n  label: Overview')];
  o.push('The API reference is generated from the widget metadata (`metadata/`), the documentation-text pass (`metadata/docs/`) and the annotations (`metadata/overrides/`). Every page shows where each fact came from.', '');
  o.push('| Toolkit | Version | Elements | Annotated & tested | Kinds ingested | Properties | Events | Methods |', '| --- | --- | --- | --- | --- | --- | --- | --- |');
  for (const ir of model.irs) {
    const c = ir.coverage;
    o.push(`| [${ir.toolkit.name}](${model.url(`/api/${ir.toolkit.id}/`)}) | ${ir.toolkit.version} | ${c.elements} | ${c.tested_kinds} | ${c.kinds} | ${c.properties} | ${c.events} | ${c.methods} |`);
  }
  o.push('', `Also: the [element support matrix](${model.url('/api/matrix/')}) shows which elements exist in which toolkit, and the [coverage dashboard](${model.url('/coverage/')}) tracks annotation progress.`, '');
  o.push(':::caution[What "annotated and tested" means]', 'Only the prototype kinds (`window`, `box`, `label`, `button`, `entry`, `scrolled-window` and a few more) are hand-annotated with generated verification tests. All other kinds are **raw**: facts straight from GIR, qdoc and the QMetaObject. See [the annotation workflow](' + model.url('/guides/annotation-workflow/') + ').', ':::', '');
  return o.join('\n');
}

export function renderMatrix(model) {
  const tags = [...model.tagIndex.keys()].sort();
  const o = [fm('Element support matrix', 'Which element tags exist in which toolkit', 'sidebar:\n  label: Support matrix\ntableOfContents: false')];
  const portable = tags.filter((t) => model.tagIndex.get(t).size > 1);
  o.push(`${tags.length} element tags. **${portable.length} are portable** (present in more than one toolkit): ${portable.map((t) => `[\`${t}\`](${model.link(model.tagIndex.get(t).values().next().value.id)})`).join(', ')}.`, '');
  const heads = [...model.toolkits.values()].map((x) => x.toolkit.name).concat(model.planned.map((p) => `${p.name} (${p.status})`));
  o.push(`| Element | ${heads.join(' | ')} |`, `| --- | ${heads.map(() => '---').join(' | ')} |`);
  for (const t of tags) {
    const m = model.tagIndex.get(t);
    const cols = [...model.toolkits.keys()].map((id) => {
      const e = m.get(id);
      return e ? `[${e.native_name}](${model.link(e.id)}) ${e.status.tier === 'annotated' ? '✔' : ''}` : '–';
    });
    o.push(`| \`${t}\` | ${cols.join(' | ')} | ${model.planned.map(() => '–').join(' | ')} |`.replace(/ \| $/, ' |'));
  }
  o.push('', '✔ = annotated and tested. Planned toolkits have no element tables yet.', '');
  return o.join('\n');
}

export function renderCoverage(model) {
  const o = [fm('Coverage dashboard', 'Kinds ingested vs annotated vs tested, members per toolkit, docs and serdes coverage', 'tableOfContents: false\nsidebar:\n  label: Coverage')];
  o.push('Everything on this page is computed at build time from the generated IR. Serdes support is read from metadata when present; until the serdes lane adds it, it shows **unknown**.', '');
  const bar = (n, d) => `<span class="svl-bar" title="${n} of ${d}"><span style="width:${d ? Math.round((100 * n) / d) : 0}%"></span></span> ${n} / ${d}`;
  for (const ir of model.irs) {
    const c = ir.coverage;
    const tk = ir.toolkit.id;
    o.push(`## ${ir.toolkit.name}`, '');
    o.push('| Measure | Value |', '| --- | --- |');
    o.push(`| Kinds ingested | ${c.kinds} (${Object.entries(c.kinds_by_type ?? {}).map(([k, v]) => `${v} ${k}`).join(', ')}) |`);
    o.push(`| Element tags | ${bar(c.elements, c.kinds)} |`);
    o.push(`| Annotated kinds | ${bar(c.annotated_kinds, c.kinds)} |`);
    o.push(`| Tested kinds | ${bar(c.tested_kinds, c.kinds)} |`);
    o.push(`| Properties | ${c.properties} (${c.annotated_properties} annotated) |`);
    o.push(`| Events | ${c.events} (${c.annotated_events} annotated) |`);
    o.push(`| Methods | ${c.methods} |`);
    if (c.dangerous) o.push(`| Dangerous members | ${c.dangerous.properties} properties, ${c.dangerous.events} events, ${c.dangerous.methods} methods (warned on their pages) |`);
    o.push(`| Properties by origin | ${Object.entries(c.properties_by_origin ?? {}).map(([k, v]) => `${badge(k)} ${v}`).join(' ')} |`);
    for (const [k, v] of Object.entries(c.docs ?? {})) o.push(`| Docs: ${k.replace(/_/g, ' ')} | ${v} |`);
    o.push(`| Serdes (per-type support) | ${c.serdes.status === 'unknown' ? '**unknown** (no serdes fields in metadata yet)' : `${c.serdes.supported_types} / ${c.serdes.total_types}`} |`);
    o.push(`| Doc text policy | \`${ir.toolkit.text_policy ?? 'full'}\` (${esc(ir.toolkit.doc_license.spdx)}) |`);
    o.push('');
    const ann = ir.entities.filter((e) => e.status.tier === 'annotated');
    o.push(`### Annotated and tested kinds (${ann.length})`, '', ann.map((e) => `[${e.tags.length ? '<' + e.tags[0] + '>' : e.name}](${model.link(e.id)})`.replace(/</g, '\\<')).join(', '), '');
    const missing = ir.entities.filter((e) => e.tags.length && e.status.tier !== 'annotated');
    if (missing.length) o.push(`### Elements still raw (${missing.length})`, '', `<details><summary>Show</summary>\n\n${missing.map((e) => `[${e.tags[0]}](${model.link(e.id)})`).join(', ')}\n\n</details>`, '');
  }
  return o.join('\n');
}

export function renderHome(model) {
  const tot = model.irs.reduce((n, ir) => n + ir.coverage.kinds, 0);
  const els = model.tagIndex.size;
  return `---
title: Svelters
description: Write stock Svelte 5, render with native toolkits (GTK 4, Qt 6, later more).
template: splash
hero:
  tagline: Stock Svelte 5 components running in embedded QuickJS, rendered by native GTK 4 and Qt Widgets. Documentation generated from the widget metadata.
  actions:
    - text: Get started
      link: ${model.url('/guides/getting-started/')}
      icon: right-arrow
    - text: API reference
      link: ${model.url('/api/')}
      variant: minimal
---

import { Card, CardGrid } from '@astrojs/starlight/components';

<CardGrid stagger>
  <Card title="${els} element tags" icon="puzzle">
    ${tot} kinds ingested across ${model.irs.length} toolkits. Each page lists props, events, slots, lifetime and provenance. [Support matrix](${model.url('/api/matrix/')}).
  </Card>
  <Card title="Honest about status" icon="approve-check">
    Annotated and tested kinds are marked; raw ingested ones are too. Planned features say so. [Coverage](${model.url('/coverage/')}).
  </Card>
  <Card title="One bundle, many toolkits" icon="rocket">
    The same JS bundle runs on GTK and Qt. [Architecture](${model.url('/guides/architecture/')}).
  </Card>
  <Card title="Contribute annotations" icon="pencil">
    Agents draft, generated tests verify. [Workflow](${model.url('/guides/annotation-workflow/')}).
  </Card>
</CardGrid>
`;
}

// Whole-site page map: relative content path -> markdown.
export function renderSite(irs, opts = {}) {
  const model = new Model(irs, opts);
  const pages = new Map();
  pages.set('api/index.md', renderApiIndex(model));
  pages.set('api/matrix.md', renderMatrix(model));
  pages.set('coverage.md', renderCoverage(model));
  pages.set('index.mdx', renderHome(model));
  for (const ir of model.irs) {
    const tk = ir.toolkit.id;
    pages.set(`api/${tk}/index.md`, renderToolkitIndex(model, ir));
    pages.set(`api/${tk}/types/index.md`, renderTypesIndex(model, ir));
    for (const e of ir.entities) {
      const route = model.routes.get(e.id); // /api/tk/x/
      pages.set(route.slice(1, -1) + '.md', renderEntity(model, e));
    }
  }
  const routes = {};
  for (const [id, p] of model.routes) routes[id] = p;
  return { pages, model, routes };
}
