// Selector vocabulary (pseudo-classes per toolkit), the Qt box quirks, token names and value
// checks. Plain ES2022 with no imports: the renderer bundles this into apps. Which properties a
// toolkit has, and how each is applied, is the style table (props.generated.js, from
// metadata/style/<toolkit>.yaml); the docs reference pages are rendered from it.

/** Separator inside the string payloads of the `style` host op. */
export const SEP = '\u0001';

export const TOOLKITS = ['gtk', 'qt'];

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
