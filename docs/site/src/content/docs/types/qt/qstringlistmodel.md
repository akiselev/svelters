---
title: QStringListModel
description: "QStringListModel from svelters:qt/core, a model class generated from qdoc, PySide6 and moc facts."
---

```js
import { QStringListModel } from 'svelters:qt/core';
```

## Class

`QStringListModel` (QtCore, `qstringlistmodel.h`), category **model**.

Lifetime: a **SHARED** handle when JS makes it (`new`): one JS object per native object (`===` works); `dispose()` or the wrapper's garbage collection deletes a parentless object, and a QObject parent that takes it over owns it from then on. Objects a getter returns are **borrowed**: JS never deletes them and the handle goes stale when Qt deletes the object.

It extends `QAbstractListModel`.

Facts: `metadata/qt/types/QtCore.json`; ledger fact ids `svelters-api:v1:type:qt:QtCore:QStringListModel:*` in `metadata/verified/qt.json`.

## Construct

`new QStringListModel({ ...props })` creates it with construct-time properties: `objectName`.

Constructors (one JS constructor; the backend picks the overload from the arguments):

- `new QStringListModel(parent)`
- `new QStringListModel(strings, parent)`

## Methods

- `clearItemData(index)` (since Qt 6.0)
- `data(index, role)`
- `flags(index)`
- `insertRows(row, count, parent)`
- `moveRows(sourceParent, sourceRow, count, destinationParent, destinationChild)`
- `removeRows(row, count, parent)`
- `rowCount(parent)`
- `setData(index, value, role)`
- `setStringList(strings)`
- `sibling(row, column, idx)`
- `sort(column, order)`
- `stringList()`
- `supportedDropActions()`

## Not bound

- `QStringListModel.itemData(const QModelIndex &)`: container
- `QStringListModel.setItemData(const QModelIndex &, const QMap<int, QVariant> &)`: container
