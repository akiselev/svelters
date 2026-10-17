// Runtime-safe surface of @svelters/style (no Node, no Svelte compiler): what the renderer bundles.
export { diag, formatDiagnostic, inlineToNative, parseDeclarations, replaceVars, translateDecls } from './decls.js';
export { KINDS } from './kinds.generated.js';
export { DEFAULT_TOKENS, paletteOf, renderGtkTokens, renderQss, resolveTokens, usesTokens } from './tokens.js';
export { DEFAULT_TOKEN_NAMES, PSEUDO, SEP, TOOLKITS } from './vocab.js';
export { STYLE_TABLES, appliesTo, byPrecedence, isSetter, parseSetterValue, propOf, ruleMatches, tagsOf, unknownProp } from './props.js';
