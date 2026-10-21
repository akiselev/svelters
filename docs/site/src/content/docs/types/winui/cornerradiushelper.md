---
title: CornerRadiusHelper
description: "Microsoft.UI.Xaml.CornerRadiusHelper from svelters:winui/microsoft.ui.xaml, a statics class generated from winmd facts."
---

```js
import { CornerRadiusHelper } from 'svelters:winui/microsoft.ui.xaml';
```

## Class

`Microsoft.UI.Xaml.CornerRadiusHelper`, category **statics**, thread class **ui**.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), holding one COM reference; `dispose()` or the wrapper's garbage collection releases it (never `Close()`: that is an explicit method).

Thread: XAML and Composition objects live on the one UI thread; a handle for an object of another thread's dispatcher is refused (NON-GUI-TYPES section 4.1).

Facts: `metadata/winui/types/Microsoft.UI.Xaml.json`; ledger fact ids `svelters-api:v1:type:winui:Microsoft.UI.Xaml:CornerRadiusHelper:*` in `metadata/verified/winui.json`.

## Construct

Not constructible from JS.

## Statics

- `CornerRadiusHelper.fromRadii(topLeft, topRight, bottomRight, bottomLeft)`
- `CornerRadiusHelper.fromUniformRadius(uniformRadius)`
