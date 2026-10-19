---
title: QTextDocument
description: "QTextDocument from svelters:qt/gui, a object class generated from qdoc, PySide6 and moc facts."
---

```js
import { QTextDocument } from 'svelters:qt/gui';
```

## Class

`QTextDocument` (QtGui, `qtextdocument.h`), category **object**.

Lifetime: a **SHARED** handle when JS makes it (`new`): one JS object per native object (`===` works); `dispose()` or the wrapper's garbage collection deletes a parentless object, and a QObject parent that takes it over owns it from then on. Objects a getter returns are **borrowed**: JS never deletes them and the handle goes stale when Qt deletes the object.

It extends `QObject`.

Facts: `metadata/qt/types/QtGui.json`; ledger fact ids `svelters-api:v1:type:qt:QtGui:QTextDocument:*` in `metadata/verified/qt.json`.

## Construct

`new QTextDocument({ ...props })` creates it with construct-time properties: `objectName`, `baseUrl`, `defaultFont`, `defaultStyleSheet`, `documentMargin`, `indentWidth`, `layoutEnabled`, `maximumBlockCount`, `modified`, `pageSize`, `textWidth`, `undoRedoEnabled`, `useDesignMetrics`.

Constructors (one JS constructor; the backend picks the overload from the arguments):

- `new QTextDocument(parent)`
- `new QTextDocument(text, parent)`

## Properties

- `baseUrl` (value)
- `blockCount` (int, read-only)
- `defaultFont` (value)
- `defaultStyleSheet` (string)
- `documentMargin` (float)
- `indentWidth` (float)
- `layoutEnabled` (bool)
- `maximumBlockCount` (int)
- `modified` (bool)
- `pageSize` (record)
- `size` (record, read-only)
- `textWidth` (float)
- `undoRedoEnabled` (bool)
- `useDesignMetrics` (bool)

## Methods

- `addResource(type, name, resource)`
- `adjustSize()`
- `allFormats()`
- `availableRedoSteps()`
- `availableUndoSteps()`
- `baselineOffset()` (since Qt 6.0)
- `begin()`
- `characterCount()`
- `clear()`
- `clearUndoRedoStacks(stacksToClear)`
- `clone(parent)`
- `defaultCursorMoveStyle()`
- `defaultTextOption()`
- `documentLayout()`
- `end()`
- `find(expr, from, options)`
- `find(subString, position, options)`
- `find(expr, cursor, options)`
- `find(subString, cursor, options)`
- `findBlock(pos)`
- `findBlockByLineNumber(lineNumber)`
- `findBlockByNumber(blockNumber)`
- `firstBlock()`
- `idealWidth()`
- `isEmpty()`
- `isLayoutEnabled()`
- `isModified()`
- `isRedoAvailable()`
- `isUndoAvailable()`
- `isUndoRedoEnabled()`
- `lastBlock()`
- `lineCount()`
- `markContentsDirty(position, length)`
- `metaInformation(info)`
- `object(objectIndex)`
- `objectForFormat(f)`
- `pageCount()`
- `redo()`
- `redoCursor()` → `{cursor}`
- `resource(type, name)`
- `revision()`
- `rootFrame()`
- `setBaseUrl(url)`
- `setBaselineOffset(baseline)` (since Qt 6.0)
- `setDefaultCursorMoveStyle(style)`
- `setDefaultFont(font)`
- `setDefaultStyleSheet(sheet)`
- `setDefaultTextOption(option)`
- `setDocumentLayout(layout)`
- `setDocumentMargin(margin)`
- `setHtml(html)`
- `setIndentWidth(width)`
- `setLayoutEnabled(b)`
- `setMarkdown(markdown, features)`
- `setMaximumBlockCount(maximum)`
- `setMetaInformation(info, string)`
- `setModified(m)`
- `setPageSize(size)`
- `setPlainText(text)`
- `setSubScriptBaseline(baseline)` (since Qt 6.0)
- `setSuperScriptBaseline(baseline)` (since Qt 6.0)
- `setTextWidth(width)`
- `setUndoRedoEnabled(enable)`
- `setUseDesignMetrics(b)`
- `subScriptBaseline()` (since Qt 6.0)
- `superScriptBaseline()` (since Qt 6.0)
- `toHtml()`
- `toMarkdown(features)`
- `toPlainText()`
- `toRawText()`
- `undo()`
- `undoCursor()` → `{cursor}`

## Signals

- `on('baseUrlChanged', (url) => ...)`
- `on('blockCountChanged', (newBlockCount) => ...)`
- `on('contentsChange', (position, charsRemoved, charsAdded) => ...)`
- `on('contentsChanged', () => ...)`
- `on('cursorPositionChanged', (cursor) => ...)`
- `on('documentLayoutChanged', () => ...)`
- `on('modificationChanged', (changed) => ...)`
- `on('redoAvailable', (available) => ...)`
- `on('undoAvailable', (available) => ...)`
- `on('undoCommandAdded', () => ...)`

## Hidden

Reached otherwise (a property) or mutators of a value class: `baseUrl`, `blockCount`, `defaultFont`, `defaultStyleSheet`, `documentMargin`, `indentWidth`, `maximumBlockCount`, `pageSize`, `size`, `textWidth`, `useDesignMetrics`.

## Not bound

- `QTextDocument.characterAt(int)`: blocked-type:QChar
- `QTextDocument.defaultResourceProvider()`: type:unknown:QTextDocument::ResourceProvider
- `QTextDocument.drawContents(QPainter *, const QRectF &)`: type:transient:QPainter
- `QTextDocument.print(QPagedPaintDevice *)`: type:transient:QPagedPaintDevice
- `QTextDocument.resourceProvider()`: type:unknown:QTextDocument::ResourceProvider
- `QTextDocument.setDefaultResourceProvider(const QTextDocument::ResourceProvider &)`: type:unknown:QTextDocument::ResourceProvider
- `QTextDocument.setResourceProvider(const QTextDocument::ResourceProvider &)`: type:unknown:QTextDocument::ResourceProvider
