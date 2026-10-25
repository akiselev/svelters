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
// MOTION-CSS: on a backend with the `native_motion` capability the compiler imports `svelte/motion`
// from `svelters:motion` (also `@svelters/native-runtime/motion`): the same API.
declare module 'svelters:motion' {
  export * from 'svelte/motion';
}
