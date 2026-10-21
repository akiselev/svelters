---
title: QTimer
description: "QTimer from svelters:qt/core, a object class generated from qdoc, PySide6 and moc facts."
---

```js
import { QTimer } from 'svelters:qt/core';
```

## Class

`QTimer` (QtCore, `qtimer.h`), category **object**.

Lifetime: a **SHARED** handle when JS makes it (`new`): one JS object per native object (`===` works); `dispose()` or the wrapper's garbage collection deletes a parentless object, and a QObject parent that takes it over owns it from then on. Objects a getter returns are **borrowed**: JS never deletes them and the handle goes stale when Qt deletes the object.

It extends `QObject`.

Facts: `metadata/qt/types/QtCore.json`; ledger fact ids `svelters-api:v1:type:qt:QtCore:QTimer:*` in `metadata/verified/qt.json`.

## Construct

`new QTimer({ ...props })` creates it with construct-time properties: `objectName`, `interval`, `singleShot`, `timerType`.

Constructors (one JS constructor; the backend picks the overload from the arguments):

- `new QTimer(parent)`

## Statics

- `QTimer.singleShot(nsec, receiver, member)`
- `QTimer.singleShot(nsec, timerType, receiver, member)`

## Properties

Getters marked *reactive* re-run the effects (and template expressions) that read them when the property changes natively (Qt announces it, the host delivers it once per event-loop turn); *reactive on writes* re-runs only after your own writes. The other getters are snapshots. To read a reactive getter without subscribing, use `obj.peek('prop')` or Svelte's `untrack(() => obj.prop)`. See [Reactivity](../../../concepts/reactivity/).

- `active` (bool, read-only, reactive)
- `interval` (int, reactive)
- `remainingTime` (int, read-only, not reactive: changes are not announced)
- `singleShot` (bool, reactive)
- `timerType` (enum, reactive)

## Methods

- `id()` (since Qt 6.8)
- `intervalAsDuration()`
- `isActive()`
- `isSingleShot()`
- `remainingTimeAsDuration()`
- `setInterval(msec)`
- `setSingleShot(singleShot)`
- `setTimerType(atype)`
- `start()`
- `start(msec)`
- `stop()`
- `timerId()`

## Signals

- `on('timeout', () => ...)`

## Hidden

Reached otherwise (a property) or mutators of a value class: `interval`, `remainingTime`, `timerType`.

## Not bound

- `QTimer.bindableActive()`: type:unknown:QBindable<bool>
- `QTimer.bindableInterval()`: type:unknown:QBindable<int>
- `QTimer.bindableSingleShot()`: type:unknown:QBindable<bool>
- `QTimer.bindableTimerType()`: type:unknown:QBindable<Qt::TimerType>
