// The portable style vocabulary and its per-toolkit support matrix. Plain ES2022 with no imports:
// the renderer bundles this into apps (inline `style`), the build plugin and tests use it for
// translation. Everything the docs say about support comes from MATRIX
// (docs/site: styling guide; test/docs.test.mjs checks the page against it).

/** Separator inside the string payloads of the `style` host op. */
export const SEP = '\u0001';

export const TOOLKITS = ['gtk', 'qt'];

/**
 * Property support. `yes`: translated; `rule`/`inline` restrict where a toolkit honours it;
 * `no`: dropped with a warning naming the toolkit. `note` is shown in the docs matrix.
 */
const BOTH = { gtk: 'yes', qt: 'yes' };
const GTK_ONLY = { gtk: 'yes', qt: 'no' };

const SIDES = ['top', 'right', 'bottom', 'left'];
const CORNERS = ['top-left', 'top-right', 'bottom-left', 'bottom-right'];

/** @type {Record<string, {gtk: string, qt: string, note?: string}>} */
export const MATRIX = {
  color: BOTH,
  background: { ...BOTH, note: 'colour only on Qt: gradients are not translated' },
  'background-color': BOTH,
  border: BOTH,
  'border-width': BOTH,
  'border-style': BOTH,
  'border-color': BOTH,
  'border-radius': BOTH,
  padding: BOTH,
  margin: { ...BOTH, note: 'layout: prefer widget props (margin-start, spacing, ...)' },
  'font-family': BOTH,
  'font-size': BOTH,
  'font-weight': BOTH,
  'font-style': BOTH,
  opacity: { gtk: 'yes', qt: 'inline', note: 'QSS has no opacity: Qt honours it for style= / style: only (QGraphicsOpacityEffect)' },
  'min-width': BOTH,
  'min-height': BOTH,
  'text-decoration': { gtk: 'yes', qt: 'no', note: 'QSS has text-decoration on some widgets only; not translated' },
  'letter-spacing': GTK_ONLY,
  'box-shadow': GTK_ONLY,
  'text-shadow': GTK_ONLY,
  transition: { ...GTK_ONLY, note: 'QSS has no transitions' },
  animation: { ...GTK_ONLY, note: 'QSS has no animations' },
};
for (const s of SIDES) {
  for (const p of ['border', 'padding', 'margin']) MATRIX[`${p}-${s}`] = MATRIX[p];
  for (const p of ['width', 'style', 'color']) MATRIX[`border-${s}-${p}`] = MATRIX[`border-${p}`];
}
for (const c of CORNERS) MATRIX[`border-${c}-radius`] = MATRIX['border-radius'];
for (const p of ['property', 'duration', 'timing-function', 'delay']) MATRIX[`transition-${p}`] = MATRIX.transition;
for (const p of ['name', 'duration', 'timing-function', 'delay', 'iteration-count', 'direction', 'fill-mode', 'play-state']) {
  MATRIX[`animation-${p}`] = MATRIX.animation;
}

/** Layout properties: refused with a pointer to the widget prop that does the job. */
export const LAYOUT_HINTS = {
  width: 'use the width-request prop (or a layout prop)',
  height: 'use the height-request prop (or a layout prop)',
  'max-width': 'there is no CSS max-width on GTK; size with widget props',
  'max-height': 'there is no CSS max-height on GTK; size with widget props',
  display: 'layout is done with widget props and container kinds, not CSS',
  position: 'layout is done with widget props and container kinds, not CSS',
  flex: 'layout is done with widget props and container kinds, not CSS',
  gap: 'use the spacing prop of the container',
  'text-align': 'use the xalign / justify prop',
  cursor: 'there is no portable cursor style',
};

/** Pseudo-classes: portable ones first, then toolkit extras. */
export const PSEUDO = {
  gtk: {
    hover: 'hover',
    focus: 'focus',
    'focus-visible': 'focus-visible',
    'focus-within': 'focus-within',
    disabled: 'disabled',
    checked: 'checked',
    active: 'active',
    'first-child': 'first-child',
    'last-child': 'last-child',
    'only-child': 'only-child',
    backdrop: 'backdrop',
    selected: 'selected',
  },
  qt: {
    hover: 'hover',
    focus: 'focus',
    'focus-visible': ['focus', ':focus-visible is approximated by :focus'],
    disabled: 'disabled',
    checked: 'checked',
    active: 'pressed',
    selected: 'selected',
  },
};

/** Qt classes whose QSS background only paints once the box is customised (a border is set). */
export const QT_NEEDS_BORDER = new Set([
  'QPushButton',
  'QToolButton',
  'QComboBox',
  'QSpinBox',
  'QDoubleSpinBox',
  'QCommandLinkButton',
  'QFontComboBox',
]);
/** Qt classes that paint a background without a border. */
export const QT_PLAIN_BOX = new Set(['QLabel', 'QWidget', 'QMainWindow', 'QFrame', 'QDialog', 'QGroupBox']);

/** The default `--sv-*` token names (values live in tokens.js). */
export const DEFAULT_TOKEN_NAMES = [
  '--sv-bg',
  '--sv-fg',
  '--sv-surface',
  '--sv-border',
  '--sv-accent',
  '--sv-accent-hover',
  '--sv-accent-fg',
  '--sv-muted',
  '--sv-danger',
  '--sv-radius',
  '--sv-space-1',
  '--sv-space-2',
  '--sv-space-3',
  '--sv-space-4',
  '--sv-font-size',
  '--sv-font-family',
];

const QT_BAD_VALUE = /oklch|oklab|lch\(|lab\(|color-mix|calc\(|clamp\(|min\(|max\(|\brem\b|gradient\(|env\(|attr\(|!important/;
const GTK_BAD_VALUE = /!important|\d%\s*$/;

/**
 * Checks `value` for `prop` against a toolkit. Returns null when it is acceptable, else the
 * reason. Qt is strict because a value QSS cannot parse clobbers earlier declarations.
 * `var(--sv-*)` is always accepted here; token resolution is checked separately.
 */
export function valueProblem(toolkit, prop, value) {
  const v = value.replace(/var\(\s*--[\w-]+\s*(?:,[^)]*)?\)/g, 'X');
  if (toolkit === 'qt') {
    const m = QT_BAD_VALUE.exec(v);
    if (m) return `value '${value}' for ${prop} uses '${m[0]}', which QSS cannot parse`;
    if (/\d%/.test(v)) return `percentage value '${value}' for ${prop} is not understood by QSS`;
  } else if (GTK_BAD_VALUE.test(v) && /^padding/.test(prop)) {
    return `percentage padding '${value}' is rejected by GTK`;
  } else if (/!important/.test(v)) {
    return `'!important' is rejected by GTK`;
  }
  return null;
}

/** Whether the matrix lets `prop` through on `toolkit` in a stylesheet rule (`inline` = style=). */
export function supports(toolkit, prop, where = 'rule') {
  const e = MATRIX[prop];
  if (!e) return false;
  const s = e[toolkit];
  return s === 'yes' || (s === 'inline' && where === 'inline');
}
