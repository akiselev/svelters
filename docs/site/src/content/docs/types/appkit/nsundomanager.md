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
- `canRedo()`: `-canRedo`
- `canUndo()`: `-canUndo`
- `disableUndoRegistration()`: `-disableUndoRegistration`
- `enableUndoRegistration()`: `-enableUndoRegistration`
- `endUndoGrouping()`: `-endUndoGrouping`
- `groupingLevel()`: `-groupingLevel`
- `isRedoing()`: `-isRedoing`
- `isUndoRegistrationEnabled()`: `-isUndoRegistrationEnabled`
- `isUndoing()`: `-isUndoing`
- `prepareWithInvocationTarget(target)`: `-prepareWithInvocationTarget:`
- `redo()`: `-redo`
- `redoActionIsDiscardable()`: `-redoActionIsDiscardable`
- `redoActionName()`: `-redoActionName`
- `redoActionUserInfoValueForKey(key)`: `-redoActionUserInfoValueForKey:`
- `redoCount()`: `-redoCount`
- `redoMenuItemTitle()`: `-redoMenuItemTitle`
- `redoMenuTitleForUndoActionName(actionName)`: `-redoMenuTitleForUndoActionName:`
- `removeAllActions()`: `-removeAllActions`
- `removeAllActionsWithTarget(target)`: `-removeAllActionsWithTarget:`
- `setActionIsDiscardable(discardable)`: `-setActionIsDiscardable:`
- `setActionName(actionName)`: `-setActionName:`
- `setActionUserInfoValueForKey(info, key)`: `-setActionUserInfoValue:forKey:`
- `undo()`: `-undo`
- `undoActionIsDiscardable()`: `-undoActionIsDiscardable`
- `undoActionName()`: `-undoActionName`
- `undoActionUserInfoValueForKey(key)`: `-undoActionUserInfoValueForKey:`
- `undoCount()`: `-undoCount`
- `undoMenuItemTitle()`: `-undoMenuItemTitle`
- `undoMenuTitleForUndoActionName(actionName)`: `-undoMenuTitleForUndoActionName:`
- `undoNestedGroup()`: `-undoNestedGroup`

## Properties

- `groupsByEvent`: KVC `groupsByEvent` (objc2)
- `levelsOfUndo`: KVC `levelsOfUndo` (objc2)
- `runLoopModes`: KVC `runLoopModes` (objc2)

## Not bound

- `registerUndoWithTarget:handler:`: callback
- `registerUndoWithTarget:selector:object:`: selector
