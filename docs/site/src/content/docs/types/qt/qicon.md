---
title: QIcon
description: "QIcon from svelters:qt/gui, a value class generated from qdoc, PySide6 and moc facts."
---

```js
import { QIcon } from 'svelters:qt/gui';
```

## Class

`QIcon` (QtGui, `qicon.h`), category **value**.

Lifetime: an immutable **VALUE** handle: a copy of the Qt value, no identity (`equals()` compares natively), a new handle per crossing, released by `dispose()` or garbage collection. Non-const members are hidden (decision Q3).

Facts: `metadata/qt/types/QtGui.json`; ledger fact ids `svelters-api:v1:type:qt:QtGui:QIcon:*` in `metadata/verified/qt.json`.

## Construct

Constructors (one JS constructor; the backend picks the overload from the arguments):

- `new QIcon()`
- `new QIcon(engine)`
- `new QIcon(pixmap)`
- `new QIcon(fileName)`

## Statics

- `QIcon.fallbackSearchPaths()`
- `QIcon.fallbackThemeName()`
- `QIcon.fromTheme(icon)` (since Qt 6.7)
- `QIcon.fromTheme(name)`
- `QIcon.fromTheme(icon, fallback)` (since Qt 6.7)
- `QIcon.fromTheme(name, fallback)`
- `QIcon.hasThemeIcon(icon)` (since Qt 6.7)
- `QIcon.hasThemeIcon(name)`
- `QIcon.setFallbackSearchPaths(paths)`
- `QIcon.setFallbackThemeName(name)`
- `QIcon.setThemeName(name)`
- `QIcon.setThemeSearchPaths(paths)`
- `QIcon.themeName()`
- `QIcon.themeSearchPaths()`

## Methods

- `actualSize(size, mode, state)`
- `availableSizes(mode, state)`
- `cacheKey()`
- `isMask()`
- `isNull()`
- `name()`
- `pixmap(size, mode, state)`
- `pixmap(extent, mode, state)`
- `pixmap(size, devicePixelRatio, mode, state)` (since Qt 6.0)
- `pixmap(w, h, mode, state)`

## Hidden

Reached otherwise (a property) or mutators of a value class: `addFile`, `addPixmap`, `setIsMask`.

## Not bound

- `QIcon.paint(QPainter *, const QRect &, Qt::Alignment, QIcon::Mode, QIcon::State)`: type:transient:QPainter
- `QIcon.paint(QPainter *, int, int, int, int, Qt::Alignment, QIcon::Mode, QIcon::State)`: type:transient:QPainter
