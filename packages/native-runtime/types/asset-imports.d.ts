// AS1: the ambient asset-import declarations (a script file: wildcard modules cannot live in a module).
// Referenced by `assets.d.ts`.

declare module '*.svg' { const ref: import('@svelters/native-runtime/assets').Asset; export default ref; }
declare module '*.png' { const ref: import('@svelters/native-runtime/assets').Asset; export default ref; }
declare module '*.jpg' { const ref: import('@svelters/native-runtime/assets').Asset; export default ref; }
declare module '*.jpeg' { const ref: import('@svelters/native-runtime/assets').Asset; export default ref; }
declare module '*.gif' { const ref: import('@svelters/native-runtime/assets').Asset; export default ref; }
declare module '*.webp' { const ref: import('@svelters/native-runtime/assets').Asset; export default ref; }
declare module '*.ico' { const ref: import('@svelters/native-runtime/assets').Asset; export default ref; }
declare module '*.ttf' { const ref: import('@svelters/native-runtime/assets').Asset; export default ref; }
declare module '*.otf' { const ref: import('@svelters/native-runtime/assets').Asset; export default ref; }
declare module '*.woff2' { const ref: import('@svelters/native-runtime/assets').Asset; export default ref; }
declare module '*.bin' { const ref: import('@svelters/native-runtime/assets').Asset; export default ref; }
declare module '*.txt' { const ref: import('@svelters/native-runtime/assets').Asset; export default ref; }
declare module '*?url' { const ref: import('@svelters/native-runtime/assets').Asset; export default ref; }
declare module '*?raw' { const text: string; export default text; }
declare module '*?inline' { const dataUrl: string; export default dataUrl; }
declare module '*?bytes' { const bytes: Uint8Array; export default bytes; }
declare module 'svelters:assets' {
  export { Asset } from '@svelters/native-runtime/assets';
  /** The handle of a ref, or of a path under the app root (`icons/gear.svg`). */
  export function asset(path: string | import('@svelters/native-runtime/assets').Asset): import('@svelters/native-runtime/assets').Asset;
  export function text(ref: string | import('@svelters/native-runtime/assets').Asset): string;
  export function bytes(ref: string | import('@svelters/native-runtime/assets').Asset): Uint8Array;
  export function isAsset(v: unknown): v is import('@svelters/native-runtime/assets').Asset;
}
