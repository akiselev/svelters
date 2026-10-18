---
title: Gdk.Texture
description: Gdk.Texture from svelters:gtk/gdk: a shared object class generated from the GIR.
---

```js
import { Texture } from 'svelters:gtk/gdk';
```

## Class

`Gdk.Texture`, GType `GdkTexture`.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), a strong reference held until the wrapper is collected or `dispose()`d. Dropping the wrapper never destroys the native object while anything else holds it.

It extends `GObject.Object` and implements `Gdk.Paintable`, `Gio.Icon`, `Gio.LoadableIcon`.

Facts come from the GIR; the tests behind them are listed in the ledger (`metadata/verified/gtk.json`, fact ids `svelters-api:v1:type:gtk:Gdk:Texture:*`).

## Statics

- `Texture.newFromBytes(bytes)`, since 4.6
- `Texture.newFromFile(file)`
- `Texture.newFromFilename(path)`, since 4.6
- `Texture.newFromResource(resourcePath)`

## Properties

- `height` (int, construct-only)
- `width` (int, construct-only)

## Methods

- `getColorState()`, since 4.16
- `getFormat()`, since 4.10
- `getHeight()`
- `getWidth()`
- `saveToPng(filename)`
- `saveToPngBytes()`, since 4.6
- `saveToTiff(filename)`, since 4.6
- `saveToTiffBytes()`, since 4.6
- `computeConcreteSize(specifiedWidth, specifiedHeight, defaultWidth, defaultHeight)`
- `getCurrentImage()`
- `getFlags()`
- `getIntrinsicAspectRatio()`
- `getIntrinsicHeight()`
- `getIntrinsicWidth()`
- `invalidateContents()`
- `invalidateSize()`
- `snapshot(snapshot, width, height)`
- `equal(icon2)`
- `hash()`
- `toString()`, since 2.20
- `load(size, cancellable)`

## Events

Listen with `obj.on('name', fn)`; it returns its disposer.

- `invalidate-contents`
- `invalidate-size`

## Not bound yet

2 members are not bound; reasons: array without length 1, struct by value 1.

