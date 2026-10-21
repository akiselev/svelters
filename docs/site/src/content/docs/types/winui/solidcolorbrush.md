---
title: SolidColorBrush
description: "Microsoft.UI.Xaml.Media.SolidColorBrush from svelters:winui/microsoft.ui.xaml.media, a object class generated from winmd facts."
---

```js
import { SolidColorBrush } from 'svelters:winui/microsoft.ui.xaml.media';
```

## Class

`Microsoft.UI.Xaml.Media.SolidColorBrush`, category **object**, thread class **dispatcher**.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), holding one COM reference; `dispose()` or the wrapper's garbage collection releases it (never `Close()`: that is an explicit method).

Thread: XAML and Composition objects live on the one UI thread; a handle for an object of another thread's dispatcher is refused (NON-GUI-TYPES section 4.1).

It extends `Microsoft.UI.Xaml.Media.Brush`.

Facts: `metadata/winui/types/Microsoft.UI.Xaml.Media.json`; ledger fact ids `svelters-api:v1:type:winui:Microsoft.UI.Xaml.Media:SolidColorBrush:*` in `metadata/verified/winui.json`.

## Construct

`new SolidColorBrush({ ...props })` activates it and sets the props: `opacity`, `relativeTransform`, `transform`, `color`.

- `new SolidColorBrush(color)`

One JS constructor: the backend picks the factory method by argument count.

## Properties

- `color` (record, reactive (DependencyProperty))
