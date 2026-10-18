---
title: Pango.AttrList
description: Pango.AttrList from svelters:gtk/pango: a value class generated from the GIR.
---

```js
import { AttrList } from 'svelters:gtk/pango';
```

## Class

`Pango.AttrList`, GType `PangoAttrList`.

Lifetime: a **VALUE** handle: an immutable snapshot with no identity. Its mutating methods are not exposed. `dispose()` releases it now, otherwise the garbage collector does.

Facts come from the GIR; the tests behind them are listed in the ledger (`metadata/verified/gtk.json`, fact ids `svelters-api:v1:type:gtk:Pango:AttrList:*`).

## Statics

- `AttrList.new()`
- `AttrList.fromString(text)`, since 1.50

## Methods

- `equal(otherList)`, since 1.46
- `getAttributes()`, since 1.44
- `getIterator()`
- `toString()`, since 1.50

## Hidden mutators

Value handles are immutable in JS; these native methods are not exposed: `change`, `insert`, `insert_before`, `splice`, `update`.

## Not bound yet

1 members are not bound; reasons: callback 1.

