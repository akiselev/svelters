---
title: Non-UI types (Qt)
description: "QtCore, QtGui and QtWidgets non-widget classes as ES module named exports (svelters:qt/core, ...)."
---

Every bound non-widget Qt class is a named export of `svelters:qt/core`, `svelters:qt/gui` or `svelters:qt/widgets`. Widgets stay elements (tags).

```svelte
<script>
  import { QTextDocument } from 'svelters:qt/gui';
  const doc = new QTextDocument({ plainText: 'Hello' });
</script>
<text-edit document={doc} />
```

- **Objects** (`QTimer`, `QStringListModel`): one JS object per QObject. What JS makes is SHARED: `dispose()` (or garbage collection) deletes a parentless object; a QObject parent takes ownership. Returned objects are borrowed.
- **Values** (`QUrl`, `QFont`, `QModelIndex`): immutable VALUE handles with `equals()`; non-const members are hidden. A `QModelIndex` is held as a persistent index.
- **Unique objects** (`QMimeDatabase`, `QTemporaryDir`): owned by JS, deleted on `dispose()`.
- **Items** (`QStandardItem`): owned by JS until a container takes them.
- **Overloads**: one JS name per C++ name; the backend picks the overload from the arguments' count and JS types.
- **Release builds** keep only what an app imports plus the types its signatures mention (`packages/types/qt/manifest.json`).

## Curated types

- [QFileInfo](qfileinfo/)
- [QIcon](qicon/)
- [QMimeDatabase](qmimedatabase/)
- [QStandardItem](qstandarditem/)
- [QStandardItemModel](qstandarditemmodel/)
- [QStringListModel](qstringlistmodel/)
- [QTemporaryDir](qtemporarydir/)
- [QTextDocument](qtextdocument/)
- [QTimer](qtimer/)
- [QUrl](qurl/)

## All bound types

- `svelters:qt/core`: `QAbstractAnimation` (object), `QAbstractItemModel` (model), `QAbstractListModel` (model), `QAbstractProxyModel` (model), `QAnimationGroup` (object), `QBuffer` (object), `QCalendar` (value), `QCollator` (unique), `QCommandLineOption` (unique), `QCommandLineParser` (unique), `QCryptographicHash` (unique), `QDir` (value), `QEasingCurve` (value), `QElapsedTimer` (value), `QFile` (object), `QFileDevice` (object), `QFileInfo` (value), `QFileSystemWatcher` (object), `QIODevice` (object), `QIdentityProxyModel` (model), `QItemSelection` (value), `QItemSelectionModel` (object), `QItemSelectionRange` (value), `QKeyCombination` (value), `QLine` (value), `QLineF` (value), `QLocale` (value), `QMimeData` (object), `QMimeDatabase` (unique), `QMimeType` (value), `QModelIndex` (value), `QObject` (object), `QParallelAnimationGroup` (object), `QPauseAnimation` (object), `QPersistentModelIndex` (value), `QProcess` (object), `QProcessEnvironment` (value), `QRegularExpression` (value), `QRegularExpressionMatch` (value), `QSaveFile` (object), `QSequentialAnimationGroup` (object), `QSettings` (object), `QSortFilterProxyModel` (model), `QStandardPaths` (statics), `QStringListModel` (model), `QTemporaryDir` (unique), `QTemporaryFile` (object), `QTimeZone` (value), `QTimer` (object), `QUrl` (value), `QUrlQuery` (value), `QUuid` (value), `QVariantAnimation` (object), `QXmlStreamAttribute` (value), `QXmlStreamAttributes` (value), `QXmlStreamNamespaceDeclaration` (value), `QXmlStreamReader` (unique), `QXmlStreamWriter` (unique)
- `svelters:qt/gui`: `QAbstractFileIconProvider` (unique), `QAbstractTextDocumentLayout` (object), `QAction` (object), `QActionGroup` (object), `QBrush` (value), `QClipboard` (object), `QColorSpace` (value), `QColorTransform` (value), `QDesktopServices` (statics), `QDoubleValidator` (object), `QFileSystemModel` (model), `QFont` (value), `QFontDatabase` (value), `QFontInfo` (value), `QFontMetrics` (value), `QFontMetricsF` (value), `QFontVariableAxis` (value), `QIcon` (value), `QIconEngine` (unique), `QImage` (value), `QImageReader` (unique), `QIntValidator` (object), `QKeySequence` (value), `QPainterPath` (value), `QPalette` (value), `QPen` (value), `QPixelFormat` (value), `QPixmap` (value), `QPolygon` (value), `QPolygonF` (value), `QRegion` (value), `QRegularExpressionValidator` (object), `QRgba64` (value), `QStandardItem` (item), `QStandardItemModel` (model), `QTextBlock` (value), `QTextBlockFormat` (value), `QTextBlockGroup` (object), `QTextCharFormat` (value), `QTextCursor` (value), `QTextDocument` (object), `QTextDocumentFragment` (value), `QTextFormat` (value), `QTextFrame` (object), `QTextFrameFormat` (value), `QTextImageFormat` (value), `QTextLength` (value), `QTextList` (object), `QTextListFormat` (value), `QTextObject` (object), `QTextOption` (value), `QTextTable` (object), `QTextTableCell` (value), `QTextTableCellFormat` (value), `QTextTableFormat` (value), `QTransform` (value), `QUndoCommand` (item), `QUndoStack` (object), `QValidator` (object)
- `svelters:qt/widgets`: `QCompleter` (object)
