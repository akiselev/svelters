---
title: PropertySet
description: "Windows.Foundation.Collections.PropertySet from svelters:winui/windows.foundation.collections, a model class generated from winmd facts."
---

```js
import { PropertySet } from 'svelters:winui/windows.foundation.collections';
```

## Class

`Windows.Foundation.Collections.PropertySet`, category **model**, thread class **any**.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), holding one COM reference; `dispose()` or the wrapper's garbage collection releases it (never `Close()`: that is an explicit method).

Facts: `metadata/winui/types/Windows.Foundation.Collections.json`; ledger fact ids `svelters-api:v1:type:winui:Windows.Foundation.Collections:PropertySet:*` in `metadata/verified/winui.json`.

## Construct

`new PropertySet({ ...props })` activates it and sets the props: .

## Properties

- `size` (int, read-only)

## Methods

- `clear()`
- `getView()`
- `hasKey(key)`
- `insert(key, value)`
- `lookup(key)`
- `remove(key)`

## Events

- `on('map-changed', (sender, event) => ...)`

## Not bound

- `First/0`: transient:Windows.Foundation.Collections.IIterator
