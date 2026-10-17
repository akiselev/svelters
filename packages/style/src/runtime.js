// Runtime-safe surface of @svelters/style (no Node, no Svelte compiler): what the renderer bundles.
export { diag, formatDiagnostic, inlineToNative, parseDeclarations, replaceVars, translateDecls } from './decls.js';
export { KINDS } from './kinds.generated.js';
export { DEFAULT_TOKENS, paletteOf, renderGtkTokens, renderQss, resolveTokens, usesTokens } from './tokens.js';
export { DEFAULT_TOKEN_NAMES, LAYOUT_HINTS, MATRIX, PSEUDO, SEP, TOOLKITS, supports } from './vocab.js';
