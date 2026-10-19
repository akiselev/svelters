// Type-checks the generated typings against the ergonomics of the plan (section 7).
import { File, ListStore, FileIcon, Cancellable } from 'svelters:gtk/gio';
import { TextBuffer, StringList, DirectoryList, FileDialog } from 'svelters:gtk/gtk';
import { Texture, Clipboard } from 'svelters:gtk/gdk';
import { AttrList, FontDescription } from 'svelters:gtk/pango';

const buffer = new TextBuffer({ text: 'Hello' });
const off: () => void = buffer.on('changed', () => {});
off();
const text: string | null = buffer.text;
buffer.text = 'x';

const store = new ListStore({ itemType: File });
store.append(File.newForPath('/tmp')!);
const n: number = store.getNItems();
store.on('items-changed', (position: number, removed: number, added: number) => void [position, removed, added]);

const names = new StringList({ strings: ['a', 'b'] });
names.append('c');

const dir = new DirectoryList({ file: File.newForPath('/usr/share'), attributes: 'standard::display-name' });
const dialog = new FileDialog({ title: 'Open', modal: true });
const icon = new FileIcon({ file: File.newForPath('/tmp/a.svg') });

const loaded = File.newForPath('/tmp/a').loadContents(null);
const bytes: Uint8Array | null = loaded.contents;
const ok: boolean = loaded.ret;

const font = FontDescription.fromString('Sans 12')!;
const family: string | null = font.getFamily();
const same: boolean = font.equals(FontDescription.fromString('Sans 12'));
const attrs = AttrList.fromString('0 5 weight bold');
// @ts-expect-error value classes hide their mutators
font.setFamily('Serif');

// N1: async members return Promises and take { signal } last; callbacks are typed functions
async function asyncUsage(parent: { readonly svlHandle: number | null }) {
  const signal = { aborted: false, addEventListener(_type: 'abort', _fn: () => void) {} };
  const r = await File.newForPath('/tmp/a').loadContentsAsync({ signal });
  const c: Uint8Array = r.contents;
  const etag: string | null = r.etagOut;
  const info = await File.newForPath('/tmp/a').queryInfoAsync('standard::size', 'none');
  const size: number = info.getSize();
  const picked: File = await dialog.open(parent);
  // @ts-expect-error the GCancellable is not part of an async signature
  await File.newForPath('/tmp/a').loadContentsAsync(new Cancellable());
  const usage = File.newForPath('/tmp').measureDiskUsage('none', null, (reporting: boolean, current: number) => void [reporting, current]);
  return [c, etag, size, picked, usage];
}

void [store, n, text, names, dir, dialog, icon, bytes, ok, family, same, attrs, Texture, Clipboard, Cancellable, asyncUsage];
