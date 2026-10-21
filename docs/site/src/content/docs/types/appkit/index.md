---
title: Non-UI types (AppKit)
description: "Foundation and AppKit non-view classes as ES module named exports (svelters:appkit/foundation, svelters:appkit/appkit)."
---

Every bound non-view Foundation/AppKit class is a named export of `svelters:appkit/foundation` or `svelters:appkit/appkit`. Views and windows stay elements (tags).

```svelte
<script>
  import { NSColor, NSFont } from 'svelters:appkit/appkit';
  const color = NSColor.colorWithSRGBRedGreenBlueAlpha(0.2, 0.4, 0.6, 1);
</script>
<text-field text-color={color} font={NSFont.boldSystemFontOfSize(15)} string-value="Hello" />
```

- **Facts** come from objc2's header-translator output (decision Q7): superclass chains, main-thread classes, objc2's safe/unsafe review, `NSError **` outs, completion blocks, nullability. The classification report is `metadata/appkit/types/report.txt`.
- **Objects** (`NSPasteboard`, `NSUndoManager`): one JS object per native object, a SHARED handle holding one strong reference. `dispose()` or garbage collection drops our reference.
- **Values** (`NSURL`, `NSColor`, `NSFont`, `NSAttributedString`): immutable objects; `equals()` is `isEqual:`; setters and mutating selectors are hidden.
- **Names**: methods join the selector's pieces in camelCase (`setString:forType:` is `setStringForType`); a trailing `error:` is dropped (the call throws a `NativeError`) and so is a completion handler (the call returns a Promise). Initializers are statics: `initWithString:` is `NSURL.newWithString(s)`.
- **Plain JS values**: `NSString` is a string, `NSNumber` a number, `NSArray`/`NSSet` arrays, `NSDictionary` objects, `NSData` a `Uint8Array`, `NSDate` a `Date`; `NSRect`/`NSSize`/`NSPoint`/`NSEdgeInsets`/`NSRange` are records.
- **Release builds** keep only what an app imports plus the types its signatures mention (`packages/types/appkit/manifest.json`).

## Curated types

- [NSAttributedString](nsattributedstring/)
- [NSColor](nscolor/)
- [NSFileManager](nsfilemanager/)
- [NSFont](nsfont/)
- [NSImage](nsimage/)
- [NSItemProvider](nsitemprovider/)
- [NSPasteboard](nspasteboard/)
- [NSURL](nsurl/)
- [NSUndoManager](nsundomanager/)
- [NSUserDefaults](nsuserdefaults/)

## Every bound type

- `svelters:appkit/foundation` (29): `NSAffineTransform`, `NSAttributedString`, `NSAttributedStringMarkdownParsingOptions`, `NSBundle`, `NSByteCountFormatter`, `NSCalendar`, `NSCharacterSet`, `NSDateComponents`, `NSDateFormatter`, `NSDecimalNumberHandler`, `NSFileManager`, `NSFileProviderService`, `NSFileVersion`, `NSFileWrapper`, `NSFormatter`, `NSItemProvider`, `NSLocale`, `NSMutableAttributedString`, `NSNumberFormatter`, `NSPersonNameComponents`, `NSProcessInfo`, `NSProgress`, `NSTimeZone`, `NSURL`, `NSURLComponents`, `NSURLQueryItem`, `NSUUID`, `NSUndoManager`, `NSUserDefaults`
- `svelters:appkit/appkit` (44): `NSAdaptiveImageGlyph`, `NSBezierPath`, `NSBitmapImageRep`, `NSColor`, `NSColorSpace`, `NSFont`, `NSFontDescriptor`, `NSGlyphGenerator`, `NSGraphicsContext`, `NSImage`, `NSImageRep`, `NSImageSymbolConfiguration`, `NSLayoutManager`, `NSMutableParagraphStyle`, `NSParagraphStyle`, `NSPasteboard`, `NSPasteboardItem`, `NSRunningApplication`, `NSScreen`, `NSShadow`, `NSSound`, `NSStringDrawingContext`, `NSTextAttachment`, `NSTextAttachmentViewProvider`, `NSTextBlock`, `NSTextContainer`, `NSTextContentManager`, `NSTextElement`, `NSTextLayoutFragment`, `NSTextLayoutManager`, `NSTextLineFragment`, `NSTextList`, `NSTextRange`, `NSTextSelection`, `NSTextSelectionNavigation`, `NSTextStorage`, `NSTextTab`, `NSTextTable`, `NSTextTableBlock`, `NSTextViewportLayoutController`, `NSTypesetter`, `NSWorkspace`, `NSWorkspaceAuthorization`, `NSWorkspaceOpenConfiguration`
