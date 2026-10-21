---
title: DataPackage
description: "Windows.ApplicationModel.DataTransfer.DataPackage from svelters:winui/windows.applicationmodel.datatransfer, a object class generated from winmd facts."
---

```js
import { DataPackage } from 'svelters:winui/windows.applicationmodel.datatransfer';
```

## Class

`Windows.ApplicationModel.DataTransfer.DataPackage`, category **object**, thread class **any**.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), holding one COM reference; `dispose()` or the wrapper's garbage collection releases it (never `Close()`: that is an explicit method).

Facts: `metadata/winui/types/Windows.ApplicationModel.DataTransfer.json`; ledger fact ids `svelters-api:v1:type:winui:Windows.ApplicationModel.DataTransfer:DataPackage:*` in `metadata/verified/winui.json`.

## Construct

`new DataPackage({ ...props })` activates it and sets the props: `requestedOperation`.

## Properties

- `properties` (object, read-only)
- `requestedOperation` (flags)
- `resourceMap` (object, read-only)

## Methods

- `getView()`
- `setApplicationLink(value)`
- `setBitmap(value)`
- `setData(formatId, value)`
- `setHtmlFormat(value)`
- `setRtf(value)`
- `setText(value)`
- `setWebLink(value)`

## Events

- `on('destroyed', (sender, args) => ...)`
- `on('operation-completed', (sender, args) => ...)`
- `on('share-canceled', (sender, args) => ...)`
- `on('share-completed', (sender, args) => ...)`

## Not bound

- `SetDataProvider/2`: delegate-parameter
- `SetStorageItems/1`: collection-argument
- `SetStorageItems/2`: collection-argument
