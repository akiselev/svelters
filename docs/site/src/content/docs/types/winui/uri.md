---
title: Uri
description: "Windows.Foundation.Uri from svelters:winui/windows.foundation, a object class generated from winmd facts."
---

```js
import { Uri } from 'svelters:winui/windows.foundation';
```

## Class

`Windows.Foundation.Uri`, category **object**, thread class **any**.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), holding one COM reference; `dispose()` or the wrapper's garbage collection releases it (never `Close()`: that is an explicit method).

Facts: `metadata/winui/types/Windows.Foundation.json`; ledger fact ids `svelters-api:v1:type:winui:Windows.Foundation:Uri:*` in `metadata/verified/winui.json`.

## Construct

- `new Uri(uri)`
- `new Uri(baseUri, relativeUri)`

One JS constructor: the backend picks the factory method by argument count.

## Statics

- `Uri.escapeComponent(toEscape)`
- `Uri.unescapeComponent(toUnescape)`

## Properties

- `absoluteCanonicalUri` (string, read-only)
- `absoluteUri` (string, read-only)
- `displayIri` (string, read-only)
- `displayUri` (string, read-only)
- `domain` (string, read-only)
- `extension` (string, read-only)
- `fragment` (string, read-only)
- `host` (string, read-only)
- `password` (string, read-only)
- `path` (string, read-only)
- `port` (int, read-only)
- `query` (string, read-only)
- `queryParsed` (object, read-only)
- `rawUri` (string, read-only)
- `schemeName` (string, read-only)
- `suspicious` (bool, read-only)
- `userName` (string, read-only)

## Methods

- `combineUri(relativeUri)`
- `equals(pUri)`
- `toString()`
