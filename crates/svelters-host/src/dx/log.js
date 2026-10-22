// svelters:log (DX1): `logger(category)`. Records go to Rust `tracing` with target `js` and the
// category as a field; `console.*` shares the sink without a category.
const host = globalThis.__svelters_host;

const fmt = (v) => {
  if (typeof v === 'string') return v;
  if (v instanceof Error) return v.stack ? String(v.stack) : String(v);
  try {
    const j = JSON.stringify(v);
    if (j !== undefined) return j;
  } catch (_) {}
  return String(v);
};

/** @param {string} category */
export function logger(category) {
  if (typeof category !== 'string' || !category) throw new TypeError('logger(category): category must be a non-empty string');
  const at = (level) => (message, fields) => {
    let text = fmt(message);
    if (fields !== undefined) text += ' ' + fmt(fields);
    host.dx('log', level + '\n' + category, text);
  };
  return { trace: at('trace'), debug: at('debug'), info: at('info'), warn: at('warn'), error: at('error') };
}
