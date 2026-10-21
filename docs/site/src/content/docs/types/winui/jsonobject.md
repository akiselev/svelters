---
title: JsonObject
description: "Windows.Data.Json.JsonObject from svelters:winui/windows.data.json, a model class generated from winmd facts."
---

```js
import { JsonObject } from 'svelters:winui/windows.data.json';
```

## Class

`Windows.Data.Json.JsonObject`, category **model**, thread class **any**.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), holding one COM reference; `dispose()` or the wrapper's garbage collection releases it (never `Close()`: that is an explicit method).

Facts: `metadata/winui/types/Windows.Data.Json.json`; ledger fact ids `svelters-api:v1:type:winui:Windows.Data.Json:JsonObject:*` in `metadata/verified/winui.json`.

## Construct

`new JsonObject({ ...props })` activates it and sets the props: .

## Statics

- `JsonObject.parse(input)`
- `JsonObject.tryParse(input)` → `{ret, result}`

## Properties

- `size` (int, read-only)
- `valueType` (enum, read-only)

## Methods

- `clear()`
- `getArray()`
- `getBoolean()`
- `getNamedArray(name)`
- `getNamedArray(name, defaultValue)`
- `getNamedBoolean(name)`
- `getNamedBoolean(name, defaultValue)`
- `getNamedNumber(name)`
- `getNamedNumber(name, defaultValue)`
- `getNamedObject(name)`
- `getNamedObject(name, defaultValue)`
- `getNamedString(name)`
- `getNamedString(name, defaultValue)`
- `getNamedValue(name)`
- `getNamedValue(name, defaultValue)`
- `getNumber()`
- `getObject()`
- `getString()`
- `getView()`
- `hasKey(key)`
- `insert(key, value)`
- `lookup(key)`
- `remove(key)`
- `setNamedValue(name, value)`
- `stringify()`
- `toString()`

## Not bound

- `First/0`: transient:Windows.Foundation.Collections.IIterator
