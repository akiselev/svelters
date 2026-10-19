// `<ListView>` (virtualized native lists): the implementation is the native runtime's (src/lists.js of
// @svelters/native-runtime); this package is the public import the compiler recognizes (lower/lists.rs).
export { ListView, ListController, diffKeys, applySplices, lis } from '@svelters/native-runtime/lists';
