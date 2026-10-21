---
title: DispatcherQueueTimer
description: "Microsoft.UI.Dispatching.DispatcherQueueTimer from svelters:winui/microsoft.ui.dispatching, a object class generated from winmd facts."
---

```js
import { DispatcherQueueTimer } from 'svelters:winui/microsoft.ui.dispatching';
```

## Class

`Microsoft.UI.Dispatching.DispatcherQueueTimer`, category **object**, thread class **any**.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), holding one COM reference; `dispose()` or the wrapper's garbage collection releases it (never `Close()`: that is an explicit method).

Facts: `metadata/winui/types/Microsoft.UI.Dispatching.json`; ledger fact ids `svelters-api:v1:type:winui:Microsoft.UI.Dispatching:DispatcherQueueTimer:*` in `metadata/verified/winui.json`.

## Construct

Not constructible from JS.

## Properties

- `interval` (timespan)
- `isRepeating` (bool)
- `isRunning` (bool, read-only)

## Methods

- `start()`
- `stop()`

## Events

- `on('tick', (sender, args) => ...)`
