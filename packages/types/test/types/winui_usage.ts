// Type-level checks of the generated WinUI typings (N4): compiled by `tsc -p test/types` (no emit).
import { Uri } from 'svelters:winui/windows.foundation';
import { PropertySet } from 'svelters:winui/windows.foundation.collections';
import { StorageFolder, FileIO, StorageFile } from 'svelters:winui/windows.storage';
import { SolidColorBrush } from 'svelters:winui/microsoft.ui.xaml.media';
import { Colors, ColorHelper } from 'svelters:winui/microsoft.ui';
import type { Color } from 'svelters:winui/windows.ui';
import { ThicknessHelper } from 'svelters:winui/microsoft.ui.xaml';
import type { Thickness } from 'svelters:winui/microsoft.ui.xaml';

const uri = new Uri('https://example.com/a?x=1');
const host: string = uri.host;
const port: number = uri.port;
const joined: Uri | null = uri.combineUri('b');
const escaped: string = Uri.escapeComponent('a b');

const set = new PropertySet();
const replaced: boolean = set.insert('k', 1);
const off: () => void = set.on('map-changed', (sender, event) => {
  void sender;
  void event?.key;
});

async function storage(dir: string): Promise<string> {
  const folder = await StorageFolder.getFolderFromPathAsync(dir);
  const file: StorageFile | null = await folder!.createFileAsync('n4.txt', 'replace-existing', { signal: null });
  await FileIO.writeTextAsync(file!, 'hello');
  const files: Array<StorageFile | null> = await folder!.getFilesAsync();
  void files;
  return FileIO.readTextAsync(file!);
}

const red: Color = Colors.red;
const custom: Color = ColorHelper.fromArgb(255, 1, 2, 3);
const brush = new SolidColorBrush({ opacity: 0.5 });
brush.color = custom;
const c: Color = brush.color;
const pad: Thickness = ThicknessHelper.fromUniformLength(4);

void [host, port, joined, escaped, replaced, off, storage, red, c, pad];
