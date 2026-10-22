// `svelters:clipboard` and `@svelters/native-runtime/clipboard` (DX2): a multi-format clipboard.
// Use from an app's tsconfig: "types": ["@svelters/native-runtime/platform"] (declares both
// `svelters:clipboard` and `svelters:shell`).

/** `'clipboard'` (default) or `'primary'` (the Linux primary selection). */
export interface ClipboardOptions {
  kind?: 'clipboard' | 'primary';
}

/** Data by type, in order of preference. Objects other than byte buffers are written as JSON. */
export type ClipboardData = Record<string, string | Uint8Array | ArrayBuffer | ArrayBufferView | unknown>;

export interface ClipboardEntry {
  readonly type: string;
  /** A string for text types (`text/*`, JSON, XML), a `Uint8Array` otherwise. */
  readonly data: string | Uint8Array;
  text(): string;
  json<T = unknown>(): T;
}

export function write(data: ClipboardData, opts?: ClipboardOptions): Promise<void>;
/** The first entry matching `accept` (types or `prefix/*` patterns, in order of preference), or null. */
export function read(accept?: string | readonly string[], opts?: ClipboardOptions): Promise<ClipboardEntry | null>;
export function readAll(accept?: string | readonly string[], opts?: ClipboardOptions): Promise<Record<string, string | Uint8Array>>;
export function formats(opts?: ClipboardOptions): Promise<string[]>;
export function writeText(text: string, opts?: ClipboardOptions): Promise<void>;
export function readText(opts?: ClipboardOptions): Promise<string>;

export interface Clipboard {
  write: typeof write;
  read: typeof read;
  readAll: typeof readAll;
  formats: typeof formats;
  writeText: typeof writeText;
  readText: typeof readText;
}
export const clipboard: Clipboard;
export default clipboard;
