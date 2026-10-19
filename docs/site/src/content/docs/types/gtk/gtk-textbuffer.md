---
title: Gtk.TextBuffer
description: "Gtk.TextBuffer from svelters:gtk/gtk, a shared object class generated from the GIR."
---

```js
import { TextBuffer } from 'svelters:gtk/gtk';
```

## Class

`Gtk.TextBuffer`, GType `GtkTextBuffer`.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), a strong reference held until the wrapper is collected or `dispose()`d. Dropping the wrapper never destroys the native object while anything else holds it.

It extends `GObject.Object`.

Facts come from the GIR; the tests behind them are listed in the ledger (`metadata/verified/gtk.json`, fact ids `svelters-api:v1:type:gtk:Gtk:TextBuffer:*`).

## Construct

```js
const x = new TextBuffer({ enableUndo: ..., tagTable: ..., text: ... });
```

Construct-time properties (construct-only ones can only be given here): `enableUndo`, `tagTable` (construct-only), `text`.

## Statics

- `TextBuffer.new(table)`

## Properties

- `canRedo` (bool, read-only)
- `canUndo` (bool, read-only)
- `cursorPosition` (int, read-only)
- `enableUndo` (bool)
- `hasSelection` (bool, read-only)
- `tagTable` (object, construct-only)
- `text` (str)

## Methods

- `addCommitNotify(flags, commitNotify)`, since 4.16
- `addMark(mark, where)`
- `addSelectionClipboard(clipboard)`
- `applyTag(tag, start, end)`
- `applyTagByName(name, start, end)`
- `backspace(iter, interactive, defaultEditable)`
- `beginIrreversibleAction()`
- `beginUserAction()`
- `copyClipboard(clipboard)`
- `createChildAnchor(iter)`
- `createMark(markName, where, leftGravity)`
- `cutClipboard(clipboard, defaultEditable)`
- `delete(start, end)`
- `deleteInteractive(startIter, endIter, defaultEditable)`
- `deleteMark(mark)`
- `deleteMarkByName(name)`
- `deleteSelection(interactive, defaultEditable)`
- `endIrreversibleAction()`
- `endUserAction()`
- `getBounds()`
- `getCanRedo()`
- `getCanUndo()`
- `getCharCount()`
- `getEnableUndo()`
- `getEndIter()`
- `getHasSelection()`
- `getInsert()`
- `getIterAtChildAnchor(anchor)`
- `getIterAtLine(lineNumber)`
- `getIterAtLineIndex(lineNumber, byteIndex)`
- `getIterAtLineOffset(lineNumber, charOffset)`
- `getIterAtMark(mark)`
- `getIterAtOffset(charOffset)`
- `getLineCount()`
- `getMark(name)`
- `getMaxUndoLevels()`
- `getModified()`
- `getSelectionBound()`
- `getSelectionBounds()`
- `getSelectionContent()`
- `getSlice(start, end, includeHiddenChars)`
- `getStartIter()`
- `getTagTable()`
- `getText(start, end, includeHiddenChars)`
- `insert(iter, text, len)`
- `insertAtCursor(text, len)`
- `insertChildAnchor(iter, anchor)`
- `insertInteractive(iter, text, len, defaultEditable)`
- `insertInteractiveAtCursor(text, len, defaultEditable)`
- `insertMarkup(iter, markup, len)`
- `insertPaintable(iter, paintable)`
- `insertRange(iter, start, end)`
- `insertRangeInteractive(iter, start, end, defaultEditable)`
- `moveMark(mark, where)`
- `moveMarkByName(name, where)`
- `pasteClipboard(clipboard, overrideLocation, defaultEditable)`
- `placeCursor(where)`
- `redo()`
- `removeAllTags(start, end)`
- `removeCommitNotify(commitNotifyHandler)`, since 4.16
- `removeSelectionClipboard(clipboard)`
- `removeTag(tag, start, end)`
- `removeTagByName(name, start, end)`
- `selectRange(ins, bound)`
- `setEnableUndo(enableUndo)`
- `setMaxUndoLevels(maxUndoLevels)`
- `setModified(setting)`
- `setText(text, len)`
- `undo()`

## Events

Listen with `obj.on('name', fn)`; it returns its disposer.

- `begin-user-action`
- `changed`
- `end-user-action`
- `mark-deleted`
- `modified-changed`
- `paste-done`
- `redo`
- `undo`

## Not bound yet

7 members are not bound; reasons: struct by value 7.

