// AS1: asset imports type as `Asset` handles (types/assets.d.ts).
import type { Asset } from '@svelters/native-runtime/assets';
import icon from './icon.svg';
import again from './icon.svg?url';
import note from './note.txt?raw';
import inline from './icon.svg?inline';
import bytes from './mesh.bin?bytes';
import { asset, text } from 'svelters:assets';

const a: Asset = icon;
const b: Asset = asset('src/icon.svg');
const s: string = a.url + again.path + note + inline + text(b) + String(a);
const n: number = bytes.length + a.size;
a.onChange((x: Asset) => x.bytes());
// @ts-expect-error a string is not an Asset
const bad: Asset = 'asset://app/x.svg';
export { s, n, bad };
