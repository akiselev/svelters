---
title: NSImage
description: "NSImage from svelters:appkit/appkit, a object class generated from objc2 facts."
---

```js
import { NSImage } from 'svelters:appkit/appkit';
```

## Class

`NSImage` (AppKit), category **object**.

Lifetime: **SHARED**: one JS object per native object, holding one strong reference (retain); `dispose()` or garbage collection drops our reference only.

Facts: `metadata/appkit/types/AppKit.json`; ledger fact ids `svelters-api:v1:type:appkit:AppKit:NSImage:*` in `metadata/verified/appkit.json`. Methods are called by selector; the JS name joins the selector's pieces.

## Construct

- `new NSImage(props?)`: `[[NSImage alloc] init]`, then the properties by KVC
- `NSImage.newByReferencingFile(fileName)`: `-[NSImage initByReferencingFile:]`
- `NSImage.newByReferencingURL(url)`: `-[NSImage initByReferencingURL:]`
- `NSImage.newWithContentsOfFile(fileName)`: `-[NSImage initWithContentsOfFile:]`
- `NSImage.newWithContentsOfURL(url)`: `-[NSImage initWithContentsOfURL:]`
- `NSImage.newWithData(data)`: `-[NSImage initWithData:]`
- `NSImage.newWithDataIgnoringOrientation(data)`: `-[NSImage initWithDataIgnoringOrientation:]`
- `NSImage.newWithPasteboard(pasteboard)`: `-[NSImage initWithPasteboard:]`
- `NSImage.newWithSize(size)`: `-[NSImage initWithSize:]`

## Statics

- `NSImage.canInitWithPasteboard(pasteboard)`: `+canInitWithPasteboard:`
- `NSImage.imageNamed(name)`: `+imageNamed:`
- `NSImage.imageTypes()`: `+imageTypes`
- `NSImage.imageUnfilteredTypes()`: `+imageUnfilteredTypes`
- `NSImage.imageWithSymbolNameBundleVariableValue(name, bundle, value)`: `+imageWithSymbolName:bundle:variableValue:`
- `NSImage.imageWithSymbolNameVariableValue(name, value)`: `+imageWithSymbolName:variableValue:`
- `NSImage.imageWithSystemSymbolNameAccessibilityDescription(name, description)`: `+imageWithSystemSymbolName:accessibilityDescription:`
- `NSImage.imageWithSystemSymbolNameVariableValueAccessibilityDescription(name, value, description)`: `+imageWithSystemSymbolName:variableValue:accessibilityDescription:`

## Methods

- `TIFFRepresentation()`: `-TIFFRepresentation`
- `TIFFRepresentationUsingCompressionFactor(comp, factor)`: `-TIFFRepresentationUsingCompression:factor:`
- `addRepresentation(imageRep)`: `-addRepresentation:`
- `addRepresentations(imageReps)`: `-addRepresentations:`
- `bestRepresentationForRectContextHints(rect, referenceContext, hints)`: `-bestRepresentationForRect:context:hints:`
- `drawAtPointFromRectOperationFraction(point, fromRect, op, delta)`: `-drawAtPoint:fromRect:operation:fraction:`
- `drawInRect(rect)`: `-drawInRect:`
- `drawInRectFromRectOperationFraction(rect, fromRect, op, delta)`: `-drawInRect:fromRect:operation:fraction:`
- `drawInRectFromRectOperationFractionRespectFlippedHints(dstSpacePortionRect, srcSpacePortionRect, op, requestedAlpha, respectContextIsFlipped, hints)`: `-drawInRect:fromRect:operation:fraction:respectFlipped:hints:`
- `drawRepresentationInRect(imageRep, rect)`: `-drawRepresentation:inRect:`
- `hitTestRectWithImageDestinationRectContextHintsFlipped(testRectDestSpace, imageRectDestSpace, context, hints, flipped)`: `-hitTestRect:withImageDestinationRect:context:hints:flipped:`
- `imageWithLocale(locale)`: `-imageWithLocale:`
- `imageWithSymbolConfiguration(configuration)`: `-imageWithSymbolConfiguration:`
- `isValid()`: `-isValid`
- `layerContentsForContentsScale(layerContentsScale)`: `-layerContentsForContentsScale:`
- `locale()`: `-locale`
- `name()`: `-name`
- `recache()`: `-recache`
- `recommendedLayerContentsScale(preferredContentsScale)`: `-recommendedLayerContentsScale:`
- `removeRepresentation(imageRep)`: `-removeRepresentation:`
- `representations()`: `-representations`
- `setName(string)`: `-setName:`
- `symbolConfiguration()`: `-symbolConfiguration`

## Properties

- `accessibilityDescription`: KVC `accessibilityDescription` (objc2)
- `alignmentRect`: KVC `alignmentRect` (objc2)
- `backgroundColor`: KVC `backgroundColor` (objc2)
- `cacheMode`: KVC `cacheMode` (objc2)
- `capInsets`: KVC `capInsets` (objc2)
- `delegate`: KVC `delegate` (objc2)
- `matchesOnMultipleResolution`: KVC `matchesOnMultipleResolution` (objc2)
- `matchesOnlyOnBestFittingAxis`: KVC `matchesOnlyOnBestFittingAxis` (objc2)
- `prefersColorMatch`: KVC `prefersColorMatch` (objc2)
- `resizingMode`: KVC `resizingMode` (objc2)
- `size`: KVC `size` (objc2)
- `template`: KVC `template` (objc2)
- `usesEPSOnResolutionMismatch`: KVC `usesEPSOnResolutionMismatch` (objc2)

## Not bound

- `CGImageForProposedRect:context:hints:`: pointer
- `imageWithSize:flipped:drawingHandler:`: callback
- `initWithCGImage:size:`: type:unknown:CGImage
- `initWithCoder:`: type:excluded:NSCoder
