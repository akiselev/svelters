---
title: NSPasteboard
description: "NSPasteboard from svelters:appkit/appkit, a object class generated from objc2 facts."
---

```js
import { NSPasteboard } from 'svelters:appkit/appkit';
```

## Class

`NSPasteboard` (AppKit), category **object**.

Lifetime: **SHARED**: one JS object per native object, holding one strong reference (retain); `dispose()` or garbage collection drops our reference only.

Facts: `metadata/appkit/types/AppKit.json`; ledger fact ids `svelters-api:v1:type:appkit:AppKit:NSPasteboard:*` in `metadata/verified/appkit.json`. Methods are called by selector; the JS name joins the selector's pieces.

## Construct

Not constructible: instances come from statics or other objects.

## Statics

- `NSPasteboard.generalPasteboard()`: `+generalPasteboard`
- `NSPasteboard.pasteboardByFilteringDataOfType(data, type)`: `+pasteboardByFilteringData:ofType:`
- `NSPasteboard.pasteboardByFilteringFile(filename)`: `+pasteboardByFilteringFile:`
- `NSPasteboard.pasteboardByFilteringTypesInPasteboard(pboard)`: `+pasteboardByFilteringTypesInPasteboard:`
- `NSPasteboard.pasteboardWithName(name)`: `+pasteboardWithName:`
- `NSPasteboard.pasteboardWithUniqueName()`: `+pasteboardWithUniqueName`
- `NSPasteboard.typesFilterableTo(type)`: `+typesFilterableTo:`

## Methods

- `addTypesOwner(newTypes, newOwner)`: `-addTypes:owner:`
- `availableTypeFromArray(types)`: `-availableTypeFromArray:`
- `canReadItemWithDataConformingToTypes(types)`: `-canReadItemWithDataConformingToTypes:`
- `canReadObjectForClassesOptions(classArray, options)`: `-canReadObjectForClasses:options:`
- `clearContents()`: `-clearContents`
- `dataForType(dataType)`: `-dataForType:`
- `declareTypesOwner(newTypes, newOwner)`: `-declareTypes:owner:`
- `detectMetadataForTypes(types)`: `-detectMetadataForTypes:completionHandler:` (async: returns a Promise; `{ signal }` cancels)
- `detectPatternsForPatterns(patterns)`: `-detectPatternsForPatterns:completionHandler:` (async: returns a Promise; `{ signal }` cancels)
- `detectValuesForPatterns(patterns)`: `-detectValuesForPatterns:completionHandler:` (async: returns a Promise; `{ signal }` cancels)
- `indexOfPasteboardItem(pasteboardItem)`: `-indexOfPasteboardItem:`
- `prepareForNewContentsWithOptions(options)`: `-prepareForNewContentsWithOptions:`
- `propertyListForType(dataType)`: `-propertyListForType:`
- `readFileContentsTypeToFile(type, filename)`: `-readFileContentsType:toFile:`
- `readFileWrapper()`: `-readFileWrapper`
- `readObjectsForClassesOptions(classArray, options)`: `-readObjectsForClasses:options:`
- `setDataForType(data, dataType)`: `-setData:forType:`
- `setPropertyListForType(plist, dataType)`: `-setPropertyList:forType:`
- `setStringForType(string, dataType)`: `-setString:forType:`
- `stringForType(dataType)`: `-stringForType:`
- `writeFileContents(filename)`: `-writeFileContents:`
- `writeFileWrapper(wrapper)`: `-writeFileWrapper:`
- `writeObjects(objects)`: `-writeObjects:`

## Properties

- `accessBehavior` (read-only): KVC `accessBehavior` (probe)
- `changeCount` (read-only): KVC `changeCount` (probe)
- `name` (read-only): KVC `name` (probe)
- `pasteboardItems` (read-only): KVC `pasteboardItems` (probe)
- `types` (read-only): KVC `types` (probe)
