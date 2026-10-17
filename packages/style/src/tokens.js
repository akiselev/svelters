// `--sv-*` design tokens: the default sets and how each toolkit receives them.
//
//   GTK: a `:root { --sv-x: v; }` rule in a host-owned provider at lower priority than the
//        component providers. Component CSS keeps `var(--sv-x)` and GTK resolves it, so a token
//        change rewrites that one small provider.
//   Qt:  QSS has no variables. Component rules are kept as templates with `${--sv-x}`
//        placeholders and rendered here with the active token set; the backend then sets one
//        application stylesheet. The same set also becomes the application QPalette so unstyled
//        widgets match (`paletteOf`).

/** Token values per colour scheme. Overrides from `setTokens` are merged on top. */
export const DEFAULT_TOKENS = {
  light: {
    '--sv-bg': '#f6f5f4',
    '--sv-fg': '#2e3436',
    '--sv-surface': '#ffffff',
    '--sv-border': '#cdc7c2',
    '--sv-accent': '#3584e4',
    '--sv-accent-hover': '#1c71d8',
    '--sv-accent-fg': '#ffffff',
    '--sv-muted': '#77767b',
    '--sv-danger': '#c01c28',
    '--sv-radius': '6px',
    '--sv-space-1': '4px',
    '--sv-space-2': '8px',
    '--sv-space-3': '12px',
    '--sv-space-4': '16px',
    '--sv-font-size': '11pt',
    '--sv-font-family': 'sans-serif',
  },
  dark: {
    '--sv-bg': '#242424',
    '--sv-fg': '#ffffff',
    '--sv-surface': '#303030',
    '--sv-border': '#4a4a4a',
    '--sv-accent': '#3584e4',
    '--sv-accent-hover': '#4a90e8',
    '--sv-accent-fg': '#ffffff',
    '--sv-muted': '#9a9996',
    '--sv-danger': '#ff7b63',
    '--sv-radius': '6px',
    '--sv-space-1': '4px',
    '--sv-space-2': '8px',
    '--sv-space-3': '12px',
    '--sv-space-4': '16px',
    '--sv-font-size': '11pt',
    '--sv-font-family': 'sans-serif',
  },
};

/** The active token set: defaults for `scheme` with `overrides` on top (null values remove). */
export function resolveTokens(scheme, overrides = {}) {
  const t = { ...(DEFAULT_TOKENS[scheme] ?? DEFAULT_TOKENS.light) };
  for (const [k, v] of Object.entries(overrides)) {
    if (v === null || v === undefined) delete t[k];
    else t[k] = String(v);
  }
  return t;
}

/** GTK token provider text. */
export function renderGtkTokens(tokens) {
  const lines = Object.entries(tokens).map(([k, v]) => `  ${k}: ${v};`);
  return `:root {\n${lines.join('\n')}\n}\n`;
}

const PLACEHOLDER = /\$\{(--[\w-]+)(?:\|([^}]*))?\}/g;

/**
 * Fills a Qt template. A placeholder with no value and no fallback drops its whole line (the
 * translator emits one declaration per line), so an undefined token never produces a value QSS
 * would clobber earlier declarations with.
 */
export function renderQss(template, tokens) {
  if (!template.includes('${')) return template;
  const out = [];
  for (const line of template.split('\n')) {
    if (!line.includes('${')) {
      out.push(line);
      continue;
    }
    let ok = true;
    const filled = line.replace(PLACEHOLDER, (_, name, fb) => {
      if (name in tokens) return tokens[name];
      if (fb !== undefined) return fb;
      ok = false;
      return '';
    });
    if (ok) out.push(filled);
  }
  return out.join('\n');
}

/** Whether a Qt template depends on tokens (and so must be re-rendered when they change). */
export const usesTokens = (template) => template.includes('${');

/** QPalette roles derived from a token set: `role=#rrggbb` pairs for the Qt backend. */
export function paletteOf(tokens) {
  const roles = {
    window: '--sv-bg',
    button: '--sv-surface',
    windowText: '--sv-fg',
    text: '--sv-fg',
    buttonText: '--sv-fg',
    base: '--sv-surface',
    highlight: '--sv-accent',
    highlightedText: '--sv-accent-fg',
    placeholderText: '--sv-muted',
  };
  const out = [];
  for (const [role, tok] of Object.entries(roles)) {
    const v = tokens[tok];
    if (v && /^#[0-9a-fA-F]{6}$/.test(v)) out.push(`${role}=${v}`);
  }
  return out;
}
