---
title: Gtk.CustomFilter
description: "Gtk.CustomFilter from svelters:gtk/gtk, a shared object class generated from the GIR."
---

```js
import { CustomFilter } from 'svelters:gtk/gtk';
```

## Class

`Gtk.CustomFilter`, GType `GtkCustomFilter`.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), a strong reference held until the wrapper is collected or `dispose()`d. Dropping the wrapper never destroys the native object while anything else holds it.

It extends `Gtk.Filter`.

Facts come from the GIR; the tests behind them are listed in the ledger (`metadata/verified/gtk.json`, fact ids `svelters-api:v1:type:gtk:Gtk:CustomFilter:*`).

## Construct

```js
const x = new CustomFilter({  });
```

Construct-time properties (construct-only ones can only be given here): .

## Statics

- `CustomFilter.new(matchFunc)`

## Methods

- `setFilterFunc(matchFunc)`; `matchFunc` is a callback (notified scope)

