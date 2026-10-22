// DX2: the `svelters:clipboard` and `svelters:shell` modules (the same modules as
// `@svelters/native-runtime/clipboard` and `/shell`). Use from an app's tsconfig:
//   "types": ["@svelters/native-runtime/platform"]
declare module 'svelters:clipboard' {
  export * from '@svelters/native-runtime/clipboard';
  export { default } from '@svelters/native-runtime/clipboard';
}
declare module 'svelters:shell' {
  export * from '@svelters/native-runtime/shell';
  export { default } from '@svelters/native-runtime/shell';
}
