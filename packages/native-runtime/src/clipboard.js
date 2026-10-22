// `svelters:clipboard` (also `@svelters/native-runtime/clipboard`), DX2: a multi-format clipboard.
//
//   import { clipboard } from 'svelters:clipboard';
//   await clipboard.write({ 'text/plain': tsv, 'text/html': html, 'application/x-cadatomic-refs+json': refs });
//   const item = await clipboard.read(['application/x-cadatomic-refs+json', 'text/plain']);  // first available wins
//   if (item) use(item.type, item.json());                                                  // null: nothing matched
//
// Data is typed: a record of `type -> data`, the types MIME-like strings in order of preference (the
// same types WG4b's `drag-type`/`drop-types` use, and the same patterns: `*`, `cad/*`). Values are
// strings, `Uint8Array`s / `ArrayBuffer`s, or anything else, written as JSON. Text types (`text/*`,
// JSON, XML) read back as strings, the rest as `Uint8Array`s.
//
// `{ kind: 'primary' }` uses the primary selection (Linux) instead of the clipboard.
//
// The host's clipboard is `HostConfig::platform` (svelters-host `platform.rs`): the system one in a
// desktop runner, an in-process one in tests and headless hosts. Rust callers (a Rust panel, a paste
// command) use the same clipboard, so data written here pastes there and the other way round. Every
// call returns a Promise (the system clipboard may become asynchronous); a failure rejects it.

const host = () => {
  const h = globalThis.__svelters_host;
  if (!h || typeof h.clipboardRead !== 'function') throw new Error('svelters:clipboard: this host has no clipboard');
  return h;
};

const kindOf = (opts) => opts?.kind ?? 'clipboard';

function encode(type, value) {
  if (typeof value === 'string') return value;
  if (value instanceof Uint8Array) return value;
  if (value instanceof ArrayBuffer) return new Uint8Array(value);
  if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  if (value === undefined) throw new TypeError(`svelters:clipboard: no data for ${type}`);
  return JSON.stringify(value);
}

function decodeText(data) {
  if (typeof data === 'string') return data;
  let s = '';
  for (const b of data) s += String.fromCharCode(b);
  try {
    return decodeURIComponent(escape(s));
  } catch {
    return s;
  }
}

/** One clipboard entry: its `type`, its `data`, and `text()`/`json()` readers. */
class ClipboardEntry {
  constructor(type, data) {
    this.type = type;
    this.data = data;
  }
  text() {
    return decodeText(this.data);
  }
  json() {
    return JSON.parse(this.text());
  }
}

const lift = (fn) => {
  try {
    return Promise.resolve(fn());
  } catch (e) {
    return Promise.reject(e);
  }
};

const asList = (accept) => (accept == null ? [] : Array.isArray(accept) ? accept.map(String) : [String(accept)]);

const matches = (pattern, type) =>
  pattern === '*' || pattern === type || (pattern.endsWith('/*') && type.startsWith(pattern.slice(0, -1)));

/** Replaces the clipboard's contents with `data` (`{ [type]: value }`, in order of preference). */
export function write(data, opts) {
  return lift(() => {
    if (data === null || typeof data !== 'object') throw new TypeError('svelters:clipboard: write takes { [type]: data }');
    host().clipboardWrite(kindOf(opts), Object.entries(data).map(([t, v]) => [t, encode(t, v)]));
  });
}

/**
 * The first entry matching `accept` (a type, a pattern, or an array of them in order of preference;
 * omitted: the first entry), or null.
 */
export function read(accept, opts) {
  return lift(() => {
    const want = asList(accept);
    const got = host().clipboardRead(kindOf(opts), want);
    const pick = want.length === 0 ? got[0] : want.map((p) => got.find(([t]) => matches(p, t))).find(Boolean);
    return pick ? new ClipboardEntry(pick[0], pick[1]) : null;
  });
}

/** Every entry (matching `accept`, if given) as `{ [type]: data }`, in the clipboard's order. */
export function readAll(accept, opts) {
  return lift(() => Object.fromEntries(host().clipboardRead(kindOf(opts), asList(accept))));
}

/** The types on the clipboard now. */
export function formats(opts) {
  return lift(() => host().clipboardFormats(kindOf(opts)));
}

/** Writes plain text only. */
export function writeText(text, opts) {
  return write({ 'text/plain': String(text) }, opts);
}

/** The plain text on the clipboard, or '' when there is none. */
export function readText(opts) {
  return read('text/plain', opts).then((e) => (e ? e.text() : ''));
}

export const clipboard = { write, read, readAll, formats, writeText, readText };
export default clipboard;
