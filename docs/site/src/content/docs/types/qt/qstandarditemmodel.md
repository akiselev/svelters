---
title: QStandardItemModel
description: "QStandardItemModel from svelters:qt/gui, a model class generated from qdoc, PySide6 and moc facts."
---

```js
import { QStandardItemModel } from 'svelters:qt/gui';
```

## Class

`QStandardItemModel` (QtGui, `qstandarditemmodel.h`), category **model**.

Lifetime: a **SHARED** handle when JS makes it (`new`): one JS object per native object (`===` works); `dispose()` or the wrapper's garbage collection deletes a parentless object, and a QObject parent that takes it over owns it from then on. Objects a getter returns are **borrowed**: JS never deletes them and the handle goes stale when Qt deletes the object.

It extends `QAbstractItemModel`.

Facts: `metadata/qt/types/QtGui.json`; ledger fact ids `svelters-api:v1:type:qt:QtGui:QStandardItemModel:*` in `metadata/verified/qt.json`.

## Construct

`new QStandardItemModel({ ...props })` creates it with construct-time properties: `objectName`, `sortRole`.

Constructors (one JS constructor; the backend picks the overload from the arguments):

- `new QStandardItemModel(parent)`
- `new QStandardItemModel(rows, columns, parent)`

## Properties

- `sortRole` (int)

## Methods

- `appendColumn(items)`
- `appendRow(item)`
- `appendRow(items)`
- `clear()`
- `clearItemData(index)`
- `columnCount(parent)`
- `data(index, role)`
- `dropMimeData(data, action, row, column, parent)`
- `findItems(text, flags, column)`
- `flags(index)`
- `hasChildren(parent)`
- `headerData(section, orientation, role)`
- `horizontalHeaderItem(column)`
- `index(row, column, parent)`
- `indexFromItem(item)`
- `insertColumn(column, parent)`
- `insertColumn(column, items)`
- `insertColumns(column, count, parent)`
- `insertRow(row, parent)`
- `insertRow(row, item)`
- `insertRow(row, items)`
- `insertRows(row, count, parent)`
- `invisibleRootItem()`
- `item(row, column)`
- `itemFromIndex(index)`
- `itemPrototype()`
- `mimeData(indexes)`
- `mimeTypes()`
- `removeColumns(column, count, parent)`
- `removeRows(row, count, parent)`
- `rowCount(parent)`
- `setColumnCount(columns)`
- `setData(index, value, role)`
- `setHeaderData(section, orientation, value, role)`
- `setHorizontalHeaderItem(column, item)`
- `setHorizontalHeaderLabels(labels)`
- `setItem(row, item)`
- `setItem(row, column, item)`
- `setItemPrototype(item)`
- `setRowCount(rows)`
- `setSortRole(role)`
- `setVerticalHeaderItem(row, item)`
- `setVerticalHeaderLabels(labels)`
- `sort(column, order)`
- `supportedDropActions()`
- `takeColumn(column)`
- `takeHorizontalHeaderItem(column)`
- `takeItem(row, column)`
- `takeRow(row)`
- `takeVerticalHeaderItem(row)`
- `verticalHeaderItem(row)`

## Signals

- `on('itemChanged', (item) => ...)`

## Hidden

Reached otherwise (a property) or mutators of a value class: `sortRole`.

## Not bound

- `QStandardItemModel.bindableSortRole()`: type:unknown:QBindable<int>
- `QStandardItemModel.itemData(const QModelIndex &)`: container
- `QStandardItemModel.multiData(const QModelIndex &, QModelRoleDataSpan)`: unbound:QModelRoleDataSpan
- `QStandardItemModel.roleNames()`: container
- `QStandardItemModel.setItemData(const QModelIndex &, const QMap<int, QVariant> &)`: container
- `QStandardItemModel.setItemRoleNames(const QHash<int, QByteArray> &)`: container
