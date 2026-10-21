---
title: NSUndoManager
description: "NSUndoManager from svelters:appkit/foundation, a object class generated from objc2 facts."
---

```js
import { NSUndoManager } from 'svelters:appkit/foundation';
```

## Class

`NSUndoManager` (Foundation), category **object** (main thread only).

Lifetime: **SHARED**: one JS object per native object, holding one strong reference (retain); `dispose()` or garbage collection drops our reference only.

Facts: `metadata/appkit/types/Foundation.json`; ledger fact ids `svelters-api:v1:type:appkit:Foundation:NSUndoManager:*` in `metadata/verified/appkit.json`. Methods are called by selector; the JS name joins the selector's pieces.

## Construct

- `new NSUndoManager(props?)`: `[[NSUndoManager alloc] init]`, then the properties by KVC

## Methods

- `beginUndoGrouping()`: `-beginUndoGrouping`
- `disableUndoRegistration()`: `-disableUndoRegistration`
- `enableUndoRegistration()`: `-enableUndoRegistration`
- `endUndoGrouping()`: `-endUndoGrouping`
- `prepareWithInvocationTarget(target)`: `-prepareWithInvocationTarget:`
- `redo()`: `-redo`
- `redoActionUserInfoValueForKey(key)`: `-redoActionUserInfoValueForKey:`
- `redoMenuTitleForUndoActionName(actionName)`: `-redoMenuTitleForUndoActionName:`
- `removeAllActions()`: `-removeAllActions`
- `removeAllActionsWithTarget(target)`: `-removeAllActionsWithTarget:`
- `setActionIsDiscardable(discardable)`: `-setActionIsDiscardable:`
- `setActionName(actionName)`: `-setActionName:`
- `setActionUserInfoValueForKey(info, key)`: `-setActionUserInfoValue:forKey:`
- `undo()`: `-undo`
- `undoActionUserInfoValueForKey(key)`: `-undoActionUserInfoValueForKey:`
- `undoMenuTitleForUndoActionName(actionName)`: `-undoMenuTitleForUndoActionName:`
- `undoNestedGroup()`: `-undoNestedGroup`

## Properties

- `canRedo` (read-only): KVC `canRedo` (probe)
- `canUndo` (read-only): KVC `canUndo` (probe)
- `groupingLevel` (read-only): KVC `groupingLevel` (probe)
- `groupsByEvent`: KVC `groupsByEvent` (objc2)
- `levelsOfUndo`: KVC `levelsOfUndo` (objc2)
- `redoActionIsDiscardable` (read-only): KVC `redoActionIsDiscardable` (probe)
- `redoActionName` (read-only): KVC `redoActionName` (probe)
- `redoCount` (read-only): KVC `redoCount` (probe)
- `redoMenuItemTitle` (read-only): KVC `redoMenuItemTitle` (probe)
- `redoing` (read-only): KVC `redoing` (probe)
- `runLoopModes`: KVC `runLoopModes` (objc2)
- `undoActionIsDiscardable` (read-only): KVC `undoActionIsDiscardable` (probe)
- `undoActionName` (read-only): KVC `undoActionName` (probe)
- `undoCount` (read-only): KVC `undoCount` (probe)
- `undoMenuItemTitle` (read-only): KVC `undoMenuItemTitle` (probe)
- `undoRegistrationEnabled` (read-only): KVC `undoRegistrationEnabled` (probe)
- `undoing` (read-only): KVC `undoing` (probe)

## Not bound

- `registerUndoWithTarget:handler:`: callback
- `registerUndoWithTarget:selector:object:`: selector
