// Type-checks the generated AppKit typings (N4): the AppKit twins of NON-GUI-TYPES section 7.
import { NSURL, NSFileManager, NSUserDefaults, NSUndoManager, NSItemProvider, NSAttributedString, NSMutableAttributedString } from 'svelters:appkit/foundation';
import { NSColor, NSFont, NSImage, NSPasteboard } from 'svelters:appkit/appkit';

const url = new NSURL('https://example.org/a');
const host: string | null = url.host();
const same: boolean = url.equals(NSURL.fileURLWithPath('/tmp'));
const made: NSURL | null = NSURL.newWithString('https://example.org/b');
const text: string = url.toString();

const color = NSColor.colorWithSRGBRedGreenBlueAlpha(0.2, 0.4, 0.6, 1);
const red: number = color.redComponent();
const lighter: NSColor | null = color.highlightWithLevel(0.5);
const font: NSFont | null = NSFont.boldSystemFontOfSize(15);
const size: number = NSFont.systemFontSize();

const image = NSImage.newWithSize({ width: 16, height: 16 })!;
const imageSize: { width: number; height: number } = image.size;

const fm = NSFileManager.defaultManager();
const names: Array<string> = fm.contentsOfDirectoryAtPath('/');

const pb = NSPasteboard.pasteboardWithUniqueName();
pb.clearContents();
const ok: boolean = pb.setStringForType('hi', 'public.utf8-plain-text');
const back: string | null = pb.stringForType('public.utf8-plain-text');

const defaults = NSUserDefaults.standardUserDefaults();
const flag: boolean = defaults.boolForKey('svelters.flag');

const undo = new NSUndoManager({ levelsOfUndo: 10 });
undo.levelsOfUndo = 20;
const canUndo: boolean = undo.canUndo();

const provider = new NSItemProvider();
const ctrl = { signal: { aborted: false, addEventListener(_t: 'abort', _fn: () => void) {} } };
const data: Promise<Uint8Array | null> = provider.loadDataRepresentationForTypeIdentifier('public.utf8-plain-text', { signal: ctrl.signal });

const attr = new NSAttributedString('Hello');
const plain: string = attr.string();
const mutable = new NSMutableAttributedString('Hello');
mutable.appendAttributedString(attr);
mutable.dispose();

void [host, same, made, text, red, lighter, font, size, imageSize, names, ok, back, flag, canUndo, data, plain];
