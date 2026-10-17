// Prototype: translate Svelte's scoped external CSS (compile(..., {css:'external'}).css.code)
// into GTK CSS and Qt QSS, with compile-time diagnostics. Table-driven; in the real pipeline
// KINDS / props would be generated from element metadata (docs/notes/styling-gtk-css-qss.md §6).
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';
const req = createRequire('/projects/svelters/packages/build/index.js');
const svm = await import(pathToFileURL(req.resolve('svelte/compiler')).href);
const sv = svm.compile ? svm : svm.default;

/** Element kind -> backend type selector and named parts ("sub-controls"). */
export const KINDS = {
  window: { gtk: 'window', qt: '.QMainWindow' },
  box: { gtk: 'box', qt: '.QWidget' },
  label: { gtk: 'label', qt: '.QLabel' },
  button: { gtk: 'button', qt: '.QPushButton', qtBoxNeedsBorder: true },
  togglebutton: { gtk: 'button.sv-k-togglebutton', qt: '.QPushButton[checkable="true"]', qtBoxNeedsBorder: true },
  entry: { gtk: 'entry', qt: '.QLineEdit' },
  checkbutton: { gtk: 'checkbutton', qt: '.QCheckBox', parts: { indicator: { gtk: ' > check', qt: '::indicator' } } },
  scale: { gtk: 'scale', qt: '.QSlider', parts: { slider: { gtk: ' > trough > slider', qt: '::handle' }, trough: { gtk: ' > trough', qt: '::groove' } } },
};
const PSEUDO = {
  gtk: { hover: 'hover', active: 'active', focus: 'focus', 'focus-visible': 'focus-visible', 'focus-within': 'focus-within',
    disabled: 'disabled', checked: 'checked', 'first-child': 'first-child', 'last-child': 'last-child', 'only-child': 'only-child',
    'nth-child': 'nth-child', backdrop: 'backdrop', selected: 'selected' },
  qt: { hover: 'hover', active: 'pressed', focus: 'focus', 'focus-visible': ['focus', 'approximated by :focus'], disabled: 'disabled',
    checked: 'checked', selected: 'selected' },
};
const PROPS = {
  gtk: /^(color|opacity|filter|font(-family|-size|-weight|-style|-variant|-stretch|-feature-settings|-variation-settings)?|letter-spacing|line-height|text-decoration(-color|-line|-style)?|text-shadow|text-transform|caret-color|-gtk-.*|transform(-origin)?|min-width|min-height|margin(-\w+)?|padding(-\w+)?|border(-\w+)*|outline(-\w+)?|box-shadow|background(-\w+)?|transition(-\w+)?|animation(-\w+)?|--.*)$/,
  qt: /^(color|font(-family|-size|-weight|-style)?|text-decoration|min-width|min-height|max-width|max-height|width|height|margin(-\w+)?|padding(-\w+)?|border(-\w+)*|outline(-\w+)?|background(-color|-image|-repeat|-position|-clip|-origin)?|selection-(background-)?color|placeholder-text-color|spacing|icon-size|image|subcontrol-(origin|position)|qproperty-.*)$/,
};

export function translate(cssText, backend, { tokens = {} } = {}) {
  const diags = [];
  const diag = (level, msg) => diags.push(`${level} [${backend}] ${msg}`);
  const ast = sv.parseCss(cssText);
  // Qt: the renderer sets the dynamic property class = "<QtClassName> <user classes...>", so QSS's
  // native `.x` (== [class~="x"]) works for our classes AND `.QPushButton` keeps working.
  const cls = (n) => `.${n}`;

  function compound(rel, ctx) {
    let out = ''; let kind = null; let tail = '';
    for (const s of rel.selectors) {
      switch (s.type) {
        case 'TypeSelector': {
          if (s.name === '*') { out += '*'; break; }
          kind = KINDS[s.name];
          if (!kind) { diag('error', `unknown element kind <${s.name}> in selector`); return null; }
          out += kind[backend]; break;
        }
        case 'ClassSelector': out += cls(s.name); break;
        case 'IdSelector': out += `#${s.name}`; break;
        case 'AttributeSelector':
          if (backend === 'gtk') { diag('error', `attribute selector [${s.name}] has no GTK equivalent`); return null; }
          out += `[${s.name}${s.matcher ?? ''}${s.value != null ? `"${s.value}"` : ''}]`; break;
        case 'NestingSelector': out += ctx.parent ?? ''; break;
        case 'PseudoClassSelector': {
          if (s.name === 'where' || s.name === 'is') {      // Svelte emits :where(.svelte-h); flatten single compound
            const inner = s.args.children;
            if (inner.length !== 1 || inner[0].children.length !== 1) { diag('error', `:${s.name}() with a list/complex argument is not supported`); return null; }
            const r = compound(inner[0].children[0], ctx); if (r == null) return null; out += r.text; break;
          }
          if (s.name === 'not') {
            const inner = s.args.children;
            const one = inner.length === 1 && inner[0].children.length === 1 ? inner[0].children[0].selectors : null;
            if (backend === 'gtk' && one) { const r = compound(inner[0].children[0], ctx); if (r == null) return null; out += `:not(${r.text})`; break; }
            if (backend === 'qt' && one?.length === 1 && one[0].type === 'PseudoClassSelector' && PSEUDO.qt[one[0].name]) {
              const m = PSEUDO.qt[one[0].name]; out += `:!${Array.isArray(m) ? m[0] : m}`; break;
            }
            diag('error', `:not(...) form not expressible in ${backend}`); return null;
          }
          let m = PSEUDO[backend][s.name];
          if (!m) { diag('error', `pseudo-class :${s.name} unsupported`); return null; }
          if (Array.isArray(m)) { diag('warning', `:${s.name} ${m[1]}`); m = m[0]; }
          const pc = `:${m}${s.args ? `(${cssText.slice(s.args.start, s.args.end)})` : ''}`; if (tail) tail += pc; else out += pc; break;
        }
        case 'PseudoElementSelector': {   // portable named parts: scale::slider -> GTK subnode / Qt sub-control
          const part = kind?.parts?.[s.name];
          if (!part) { diag('error', `::${s.name} is not a known part of this element`); return null; }
          tail = part[backend]; break;
        }
        default: diag('error', `selector node ${s.type} unsupported`); return null;
      }
    }
    // GTK subnodes are descendants, so pseudo-classes written after ::part must move onto the part
    return { text: out, tail, kind };
  }

  function selector(complex, ctx) {
    let out = ''; let kind = null;
    for (const rel of complex.children) {
      const comb = rel.combinator?.name;
      if (comb && backend === 'qt' && (comb === '+' || comb === '~')) { diag('error', `combinator '${comb}' never matches in QSS`); return null; }
      const c = compound(rel, ctx); if (c == null) return null;
      const t = c;
      out += (comb ? (comb === ' ' ? ' ' : ` ${comb} `) : '') + t.text + t.tail; kind = t.kind ?? kind;
    }
    return { text: out, kind };
  }

  function value(prop, v) {
    if (/var\(/.test(v)) {
      if (backend === 'gtk') return v;   // GTK resolves var() natively (tokens come from a host provider)
      // Qt: keep a placeholder; the host substitutes token values (from QPalette / token set) at runtime
      return v.replace(/var\(\s*(--[\w-]+)\s*(?:,\s*([^)]+))?\)/g, (_, n, fb) => {
        if (!(n in tokens) && !fb) diag('error', `token ${n} is not defined in the token set`);
        return `\${${n}}`;
      });
    }
    if (backend === 'qt' && /oklch|oklab|color-mix|calc\(|\brem\b|\bem\b/.test(v)) diag('error', `value '${v}' for ${prop} is not understood by QSS (would clobber)`);
    return v;
  }

  function rule(node, ctx, out) {
    const sels = [];
    for (const cx of node.prelude.children) { const s = selector(cx, ctx); if (s) sels.push(s); }
    const decls = []; const nested = [];
    for (const ch of node.block.children) {
      if (ch.type === 'Declaration') {
        if (backend === 'qt' && ch.property.startsWith('--')) { diag('error', `custom property ${ch.property} cannot be declared in QSS; put it in the token set`); continue; }
        if (!PROPS[backend].test(ch.property)) { diag('warning', `property '${ch.property}' dropped (unsupported in ${backend})`); continue; }
        decls.push(`${ch.property}: ${value(ch.property, ch.value)};`);
      } else nested.push(ch);
    }
    if (backend === 'qt' && decls.some((d) => d.startsWith('background')) && !decls.some((d) => /^border(-(top|right|bottom|left))?(-(width|style))?:/.test(d)) &&
        sels.some((s) => s.kind?.qtBoxNeedsBorder)) {
      diag('warning', `background on a push button needs a border in QSS; injected 'border: none'`); decls.push('border: none;');
    }
    if (sels.length && decls.length) out.push(`${sels.map((s) => s.text).join(', ')} { ${decls.join(' ')} }`);
    for (const n of nested) {
      if (n.type === 'Rule') for (const s of sels) rule(n, { parent: s.text }, out);   // flatten nesting
      else atrule(n, out);
    }
  }

  const variants = { base: [], dark: [] };
  function atrule(node, out) {
    if (node.name === 'media') {
      if (/prefers-color-scheme:\s*dark/.test(node.prelude)) {
        if (backend === 'gtk') { const inner = []; node.block.children.forEach((c) => rule(c, {}, inner)); out.push(`@media ${node.prelude} { ${inner.join(' ')} }`); }
        else node.block.children.forEach((c) => rule(c, {}, variants.dark));   // Qt: separate dark sheet appended by the host
        return;
      }
      diag('error', `@media ${node.prelude} unsupported`); return;
    }
    if (node.name === 'keyframes') {
      if (backend === 'gtk') out.push(cssText.slice(node.start, node.end)); else diag('warning', `@keyframes dropped (no animations in QSS)`);
      return;
    }
    diag('error', `@${node.name} unsupported`);
  }
  for (const n of ast.children) (n.type === 'Rule' ? rule(n, {}, variants.base) : atrule(n, variants.base));
  return { css: variants.base.join('\n'), dark: variants.dark.join('\n'), diagnostics: diags };
}

export function compileComponent(source, filename = 'App.svelte') {
  return sv.compile(source, { filename, css: 'external', experimental: { customRenderer: '/projects/svelters/packages/renderer/src/index.js' } });
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const fs = await import('node:fs');
  const src = fs.readFileSync(process.argv[2], 'utf8');
  const r = compileComponent(src);
  const tokens = { '--sv-accent': 1, '--sv-accent-hover': 1, '--sv-accent-fg': 1, '--sv-surface': 1, '--sv-border': 1, '--sv-fg': 1 };
  const out = { svelte_css: r.css.code, hash: (r.css.code.match(/svelte-[a-z0-9]+/) ?? [])[0],
    gtk: translate(r.css.code, 'gtk', { tokens }), qt: translate(r.css.code, 'qt', { tokens }) };
  console.log(JSON.stringify(out, null, 1));
}
