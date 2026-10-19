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

## Async methods

Each folds a native `_async`/`_finish` pair into one call that returns a Promise. It resolves with the finish result (an object of the out parameters where there are any) and rejects with a `NativeError`. The native `GCancellable` is not exposed: pass `{ signal }` (an `AbortSignal`) as the last argument to cancel, which rejects with code `cancelled`. See [Async calls and callbacks](../async/).

- `readAsync(mimeTypes, ioPriority, { signal }?)` → `Promise<{ ret, outMimeType }>` (`read_async` + `read_finish`)
- `readTextAsync({ signal }?)` → `Promise<str>` (`read_text_async` + `read_text_finish`)
- `readTextureAsync({ signal }?)` → `Promise<object>` (`read_texture_async` + `read_texture_finish`)
- `storeAsync(ioPriority, { signal }?)` → `Promise<bool>` (`store_async` + `store_finish`)

## Events

Listen with `obj.on('name', fn)`; it returns its disposer.

- `changed`

## Not bound yet

3 members are not bound; reasons: struct by value 1, variant 2.

