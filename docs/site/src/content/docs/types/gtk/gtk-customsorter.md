---
title: Gtk.CustomSorter
description: "Gtk.CustomSorter from svelters:gtk/gtk, a shared object class generated from the GIR."
---

```js
import { CustomSorter } from 'svelters:gtk/gtk';
```

## Class

`Gtk.CustomSorter`, GType `GtkCustomSorter`.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), a strong reference held until the wrapper is collected or `dispose()`d. Dropping the wrapper never destroys the native object while anything else holds it.

It extends `Gtk.Sorter`.

Facts come from the GIR; the tests behind them are listed in the ledger (`metadata/verified/gtk.json`, fact ids `svelters-api:v1:type:gtk:Gtk:CustomSorter:*`).

## Construct

```js
const x = new CustomSorter({  });
```

Construct-time properties (construct-only ones can only be given here): .

## Statics

- `CustomSorter.new(sortFunc)`

## Methods

- `setSortFunc(sortFunc)`; `sortFunc` is a callback (notified scope)

