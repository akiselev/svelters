---
title: Gdk.Clipboard
description: "Gdk.Clipboard from svelters:gtk/gdk, a shared object class generated from the GIR."
---

```js
import { Clipboard } from 'svelters:gtk/gdk';
```

## Class

`Gdk.Clipboard`, GType `GdkClipboard`.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), a strong reference held until the wrapper is collected or `dispose()`d. Dropping the wrapper never destroys the native object while anything else holds it.

It extends `GObject.Object`.

Facts come from the GIR; the tests behind them are listed in the ledger (`metadata/verified/gtk.json`, fact ids `svelters-api:v1:type:gtk:Gdk:Clipboard:*`).

## Construct

```js
const x = new Clipboard({ display: ... });
```

Construct-time properties (construct-only ones can only be given here): `display` (construct-only).

## Properties

- `content` (object, read-only)
- `display` (object, construct-only)
- `local` (bool, read-only)

## Methods

- `getContent()`
- `getDisplay()`
- `getFormats()`
- `isLocal()`
- `setContent(provider)`

## Events

Listen with `obj.on('name', fn)`; it returns its disposer.

- `changed`

## Not bound yet

7 members are not bound; reasons: async 5, struct by value 1, variant 1.

