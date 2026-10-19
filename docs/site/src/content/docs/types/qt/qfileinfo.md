---
title: QFileInfo
description: "QFileInfo from svelters:qt/core, a value class generated from qdoc, PySide6 and moc facts."
---

```js
import { QFileInfo } from 'svelters:qt/core';
```

## Class

`QFileInfo` (QtCore, `qfileinfo.h`), category **value**.

Lifetime: an immutable **VALUE** handle: a copy of the Qt value, no identity (`equals()` compares natively), a new handle per crossing, released by `dispose()` or garbage collection. Non-const members are hidden (decision Q3).

Facts: `metadata/qt/types/QtCore.json`; ledger fact ids `svelters-api:v1:type:qt:QtCore:QFileInfo:*` in `metadata/verified/qt.json`.

## Construct

Constructors (one JS constructor; the backend picks the overload from the arguments):

- `new QFileInfo()`
- `new QFileInfo(path)`
- `new QFileInfo(dir, path)`

## Statics

- `QFileInfo.exists(path)`

## Methods

- `absoluteDir()`
- `absoluteFilePath()`
- `absolutePath()`
- `baseName()`
- `birthTime()`
- `birthTime(tz)` (since Qt 6.6)
- `bundleName()`
- `caching()`
- `canonicalFilePath()`
- `canonicalPath()`
- `completeBaseName()`
- `completeSuffix()`
- `dir()`
- `exists()`
- `fileName()`
- `filePath()`
- `fileTime(time)`
- `fileTime(time, tz)` (since Qt 6.6)
- `group()`
- `groupId()`
- `isAbsolute()`
- `isAlias()` (since Qt 6.4)
- `isBundle()`
- `isDir()`
- `isExecutable()`
- `isFile()`
- `isHidden()`
- `isJunction()`
- `isNativePath()`
- `isOther()` (since Qt 6.10)
- `isReadable()`
- `isRelative()`
- `isRoot()`
- `isShortcut()`
- `isSymLink()`
- `isSymbolicLink()`
- `isWritable()`
- `junctionTarget()` (since Qt 6.2)
- `lastModified()`
- `lastModified(tz)` (since Qt 6.6)
- `lastRead()`
- `lastRead(tz)` (since Qt 6.6)
- `metadataChangeTime()`
- `metadataChangeTime(tz)` (since Qt 6.6)
- `owner()`
- `ownerId()`
- `path()`
- `permission(permissions)`
- `permissions()`
- `readSymLink()` (since Qt 6.6)
- `size()`
- `suffix()`
- `symLinkTarget()`

## Hidden

Reached otherwise (a property) or mutators of a value class: `makeAbsolute`, `refresh`, `setCaching`, `setFile`, `stat`.

## Not bound

- `QFileInfo.QFileInfo(const QFileDevice &)`: QObject by value:QFileDevice
- `QFileInfo.setFile(const QFileDevice &)`: QObject by value:QFileDevice
