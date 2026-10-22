// The per-toolkit styling reference pages (api/styling/<toolkit>.md), rendered from the style
// table (packages/style/src/props.generated.js, generated from metadata/style/<toolkit>.yaml).
import { STYLE_TABLES, TAG_SETS } from '../../../../packages/style/src/props.generated.js';

const NAMES = { gtk: 'GTK 4', qt: 'Qt 6', appkit: 'AppKit', winui: 'WinUI 3' };
const DIALECT = {
  css: 'GTK CSS',
  qss: 'QSS (Qt stylesheets)',
  none: 'none (setters only)',
  decl: 'decl (no stylesheet language: the runtime cascade engine matches the rules and the backend applies each declaration natively)',
};
const TYPE = {
  length: 'length (unitless or px)',
  int: 'integer',
  number: 'number',
  bool: '`true` / `false`',
  enum: 'one of the values',
  insets: '1 to 4 lengths (CSS order)',
  string: 'string',
  color: 'colour (hex, rgb(), hsl(), basic names)',
  border: 'width, solid / none, colour',
  shadow: 'x y [blur] colour',
  'font-weight': 'normal, bold or 100 to 900',
};

const esc = (s) => String(s).replace(/\|/g, '\\|');

/** How a property is applied, as one cell. */
export function appliedThrough(tk, e) {
  if (e.via === 'css' && STYLE_TABLES[tk]?.dialect === 'decl') return 'native (decl)';
  if (e.via === 'css') return `${tk === 'qt' ? 'QSS' : tk === 'gtk' ? 'GTK CSS' : 'CSS'}${e.inlineOnly ? ' (`style=` / `style:` only)' : ''}`;
  if (e.via === 'setter') return `setter \`${e.prop}\``;
  return `child attribute \`${e.attr}\``;
}

function appliesTo(e) {
  if (e.via === 'css') return '';
  const kinds = (e.via === 'setter' ? e.kinds : e.parents).map((k) => `\`${k}\``).join(', ');
  const n = TAG_SETS[e.tags].length;
  return e.via === 'setter' ? `${kinds} (${n} element${n === 1 ? '' : 's'})` : `children of ${kinds}`;
}

function values(e) {
  if (e.via === 'css') return '';
  if (e.type === 'enum') return e.values.map((v) => `\`${v}\``).join(', ');
  return TYPE[e.type] ?? e.type;
}

/** `Map<relative path, markdown>` of the reference pages. */
export function renderStylingPages() {
  const pages = new Map();
  for (const [tk, t] of Object.entries(STYLE_TABLES)) {
    const props = Object.entries(t.props);
    const css = props.filter(([, e]) => e.via === 'css');
    const set = props.filter(([, e]) => e.via !== 'css');
    const lines = [
      '---',
      `title: ${NAMES[tk] ?? tk} styling properties`,
      `description: "Every style property of the ${NAMES[tk] ?? tk} backend and how it is applied: ${DIALECT[t.dialect]} or a widget setter."`,
      '---',
      '',
      `Generated from \`metadata/style/${tk}.yaml\`. Names are the toolkit's own, in kebab-case; the same name can mean something else on another toolkit, and there are no portable aliases. How the layers fit together: [Styling](/guides/styling/).`,
      '',
      `- Native styling dialect: ${DIALECT[t.dialect]}.`,
      `- Setter rules take ${t.setterSelectors.map((s) => (s === 'type' ? 'element names' : s === 'class' ? '`.class`' : '`#id`')).join(', ')} in one compound selector.`,
      `- Lengths: unitless or in ${Object.keys(t.units).join(', ')}.`,
      '',
      `## Setter properties`,
      '',
      `Written with \`setProp\` (or \`setChildAttr\` on the parent) when a rule matches; the value read before the first override comes back when no rule sets the property any more.`,
      '',
      '| Property | Applied through | Applies to | Values | Notes |',
      '| --- | --- | --- | --- | --- |',
      ...set.map(([n, e]) => `| \`${n}\` | ${appliedThrough(tk, e)} | ${appliesTo(e)} | ${esc(values(e))} | ${esc(e.note ?? '')} |`),
      '',
      ...(t.dialect === 'decl'
        ? [
            '## Native properties (decl dialect)',
            '',
            "Matched by the runtime cascade engine against the element tree (descendant and child combinators, `:hover`, `:active`, `:focus`, `:disabled`, `:checked` on the last compound, `@media (prefers-color-scheme: dark)`, `var(--sv-*)` tokens) and applied by the backend; `color` and `font-*` are inherited as in CSS.",
            '',
            '| Property | Applied through | Values | Notes |',
            '| --- | --- | --- | --- |',
            ...css.map(([n, e]) => `| \`${n}\` | ${appliedThrough(tk, e)} | ${esc(e.type === 'enum' ? e.values.map((v) => `\`${v}\``).join(', ') : (TYPE[e.type] ?? e.type ?? ''))} | ${esc(e.note ?? '')} |`),
            '',
          ]
        : [
            `## ${t.dialect === 'qss' ? 'QSS' : 'CSS'} properties`,
            '',
            `Translated to ${DIALECT[t.dialect]} and applied by the toolkit's style engine (states, \`@media\` and \`@native\` work here).`,
            '',
            '| Property | Applied through | Notes |',
            '| --- | --- | --- |',
            ...css.map(([n, e]) => `| \`${n}\` | ${appliedThrough(tk, e)} | ${esc(e.note ?? '')} |`),
            '',
          ]),
    ];
    pages.set(`api/styling/${tk}.md`, lines.join('\n'));
  }
  return pages;
}
