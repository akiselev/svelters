---
title: QStandardItem
description: "QStandardItem from svelters:qt/gui, a item class generated from qdoc, PySide6 and moc facts."
---

```js
import { QStandardItem } from 'svelters:qt/gui';
```

## Class

`QStandardItem` (QtGui, `qstandarditemmodel.h`), category **item**.

Lifetime: an **item**: owned by JS until a container takes it (`appendRow`, `setItem`, `push`; the PySide ownership facts say which), then borrowed; a `take*` result gives it back. Items JS cannot watch for deletion are never handed out.

Facts: `metadata/qt/types/QtGui.json`; ledger fact ids `svelters-api:v1:type:qt:QtGui:QStandardItem:*` in `metadata/verified/qt.json`.

## Construct

Constructors (one JS constructor; the backend picks the overload from the arguments):

- `new QStandardItem()`
- `new QStandardItem(text)`
- `new QStandardItem(rows, columns)`
- `new QStandardItem(icon, text)`

## Methods

- `accessibleDescription()`
- `accessibleText()`
- `appendColumn(items)`
- `appendRow(item)`
- `appendRow(items)`
- `appendRows(items)`
- `background()`
- `checkState()`
- `child(row, column)`
- `clearData()`
- `clone()`
- `column()`
- `columnCount()`
- `data(role)`
- `flags()`
- `font()`
- `foreground()`
- `hasChildren()`
- `icon()`
- `index()`
- `insertColumn(column, items)`
- `insertColumns(column, count)`
- `insertRow(row, item)`
- `insertRow(row, items)`
- `insertRows(row, count)`
- `insertRows(row, items)`
- `isAutoTristate()`
- `isCheckable()`
- `isDragEnabled()`
- `isDropEnabled()`
- `isEditable()`
- `isEnabled()`
- `isSelectable()`
- `isUserTristate()`
- `model()`
- `removeColumn(column)`
- `removeColumns(column, count)`
- `removeRow(row)`
- `removeRows(row, count)`
- `row()`
- `rowCount()`
- `setAccessibleDescription(accessibleDescription)`
- `setAccessibleText(accessibleText)`
- `setAutoTristate(tristate)`
- `setBackground(brush)`
- `setCheckState(state)`
- `setCheckable(checkable)`
- `setChild(row, item)`
- `setChild(row, column, item)`
- `setColumnCount(columns)`
- `setData(value, role)`
- `setDragEnabled(dragEnabled)`
- `setDropEnabled(dropEnabled)`
- `setEditable(editable)`
- `setEnabled(enabled)`
- `setFlags(flags)`
- `setFont(font)`
- `setForeground(brush)`
- `setIcon(icon)`
- `setRowCount(rows)`
- `setSelectable(selectable)`
- `setSizeHint(size)`
- `setStatusTip(statusTip)`
- `setText(text)`
- `setTextAlignment(alignment)`
- `setToolTip(toolTip)`
- `setUserTristate(tristate)`
- `setWhatsThis(whatsThis)`
- `sizeHint()`
- `sortChildren(column, order)`
- `statusTip()`
- `takeChild(row, column)`
- `takeColumn(column)`
- `takeRow(row)`
- `text()`
- `textAlignment()`
- `toolTip()`
- `type()`
- `whatsThis()`

## Not bound

- `QStandardItem.multiData(QModelRoleDataSpan)`: unbound:QModelRoleDataSpan
- `QStandardItem.read(QDataStream &)`: unbound:QDataStream
- `QStandardItem.write(QDataStream &)`: unbound:QDataStream
