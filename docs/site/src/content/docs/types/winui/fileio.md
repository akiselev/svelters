---
title: FileIO
description: "Windows.Storage.FileIO from svelters:winui/windows.storage, a statics class generated from winmd facts."
---

```js
import { FileIO } from 'svelters:winui/windows.storage';
```

## Class

`Windows.Storage.FileIO`, category **statics**, thread class **any**.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), holding one COM reference; `dispose()` or the wrapper's garbage collection releases it (never `Close()`: that is an explicit method).

Facts: `metadata/winui/types/Windows.Storage.json`; ledger fact ids `svelters-api:v1:type:winui:Windows.Storage:FileIO:*` in `metadata/verified/winui.json`.

## Construct

Not constructible from JS.

## Statics

- `FileIO.appendTextAsync(file, contents)` (async: a Promise; `{ signal }` cancels)
- `FileIO.appendTextAsync(file, contents, encoding)` (async: a Promise; `{ signal }` cancels)
- `FileIO.readBufferAsync(file)` (async: a Promise; `{ signal }` cancels)
- `FileIO.readLinesAsync(file)` (async: a Promise; `{ signal }` cancels)
- `FileIO.readLinesAsync(file, encoding)` (async: a Promise; `{ signal }` cancels)
- `FileIO.readTextAsync(file)` (async: a Promise; `{ signal }` cancels)
- `FileIO.readTextAsync(file, encoding)` (async: a Promise; `{ signal }` cancels)
- `FileIO.writeBufferAsync(file, buffer)` (async: a Promise; `{ signal }` cancels)
- `FileIO.writeTextAsync(file, contents)` (async: a Promise; `{ signal }` cancels)
- `FileIO.writeTextAsync(file, contents, encoding)` (async: a Promise; `{ signal }` cancels)

## Not bound

- `static:AppendLinesAsync/2`: collection-argument
- `static:AppendLinesAsync/3`: collection-argument
- `static:WriteBytesAsync/2`: array
- `static:WriteLinesAsync/2`: collection-argument
- `static:WriteLinesAsync/3`: collection-argument
