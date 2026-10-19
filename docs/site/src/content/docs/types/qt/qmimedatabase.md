---
title: QMimeDatabase
description: "QMimeDatabase from svelters:qt/core, a unique class generated from qdoc, PySide6 and moc facts."
---

```js
import { QMimeDatabase } from 'svelters:qt/core';
```

## Class

`QMimeDatabase` (QtCore, `qmimedatabase.h`), category **unique**.

Lifetime: a JS-owned object (**OWNED**): it is deleted on `dispose()` or when the wrapper is collected. Qt never returns one JS did not make.

Facts: `metadata/qt/types/QtCore.json`; ledger fact ids `svelters-api:v1:type:qt:QtCore:QMimeDatabase:*` in `metadata/verified/qt.json`.

## Construct

Constructors (one JS constructor; the backend picks the overload from the arguments):

- `new QMimeDatabase()`

## Methods

- `allMimeTypes()`
- `mimeTypeForData(device)`
- `mimeTypeForData(data)`
- `mimeTypeForFile(fileInfo, mode)`
- `mimeTypeForFile(fileName, mode)`
- `mimeTypeForFileNameAndData(fileName, device)`
- `mimeTypeForFileNameAndData(fileName, data)`
- `mimeTypeForName(nameOrAlias)`
- `mimeTypeForUrl(url)`
- `mimeTypesForFileName(fileName)`
- `suffixForFileName(fileName)`
