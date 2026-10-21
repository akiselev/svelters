---
title: NSFont
description: "NSFont from svelters:appkit/appkit, a value class generated from objc2 facts."
---

```js
import { NSFont } from 'svelters:appkit/appkit';
```

## Class

`NSFont` (AppKit), category **value**.

Lifetime: an immutable Objective-C object behind a **SHARED** handle (one strong reference). Sharing an immutable object is indistinguishable from copying it, so there is no identity contract: `equals()` uses `isEqual:`, `toString()` is `description`. Setters and mutating selectors are hidden (decision Q3).

Facts: `metadata/appkit/types/AppKit.json`; ledger fact ids `svelters-api:v1:type:appkit:AppKit:NSFont:*` in `metadata/verified/appkit.json`. Methods are called by selector; the JS name joins the selector's pieces.

## Construct

- `new NSFont(props?)`: `[[NSFont alloc] init]`, then the properties by KVC

## Statics

- `NSFont.boldSystemFontOfSize(fontSize)`: `+boldSystemFontOfSize:`
- `NSFont.controlContentFontOfSize(fontSize)`: `+controlContentFontOfSize:`
- `NSFont.fontWithDescriptorSize(fontDescriptor, fontSize)`: `+fontWithDescriptor:size:`
- `NSFont.fontWithDescriptorTextTransform(fontDescriptor, textTransform)`: `+fontWithDescriptor:textTransform:`
- `NSFont.fontWithNameSize(fontName, fontSize)`: `+fontWithName:size:`
- `NSFont.labelFontOfSize(fontSize)`: `+labelFontOfSize:`
- `NSFont.labelFontSize()`: `+labelFontSize`
- `NSFont.menuBarFontOfSize(fontSize)`: `+menuBarFontOfSize:`
- `NSFont.menuFontOfSize(fontSize)`: `+menuFontOfSize:`
- `NSFont.messageFontOfSize(fontSize)`: `+messageFontOfSize:`
- `NSFont.monospacedDigitSystemFontOfSizeWeight(fontSize, weight)`: `+monospacedDigitSystemFontOfSize:weight:`
- `NSFont.monospacedSystemFontOfSizeWeight(fontSize, weight)`: `+monospacedSystemFontOfSize:weight:`
- `NSFont.paletteFontOfSize(fontSize)`: `+paletteFontOfSize:`
- `NSFont.preferredFontForTextStyleOptions(style, options)`: `+preferredFontForTextStyle:options:`
- `NSFont.setUserFixedPitchFont(font)`: `+setUserFixedPitchFont:`
- `NSFont.setUserFont(font)`: `+setUserFont:`
- `NSFont.smallSystemFontSize()`: `+smallSystemFontSize`
- `NSFont.systemFontOfSize(fontSize)`: `+systemFontOfSize:`
- `NSFont.systemFontOfSizeWeight(fontSize, weight)`: `+systemFontOfSize:weight:`
- `NSFont.systemFontOfSizeWeightWidth(fontSize, weight, width)`: `+systemFontOfSize:weight:width:`
- `NSFont.systemFontSize()`: `+systemFontSize`
- `NSFont.systemFontSizeForControlSize(controlSize)`: `+systemFontSizeForControlSize:`
- `NSFont.titleBarFontOfSize(fontSize)`: `+titleBarFontOfSize:`
- `NSFont.toolTipsFontOfSize(fontSize)`: `+toolTipsFontOfSize:`
- `NSFont.userFixedPitchFontOfSize(fontSize)`: `+userFixedPitchFontOfSize:`
- `NSFont.userFontOfSize(fontSize)`: `+userFontOfSize:`

## Methods

- `advancementForCGGlyph(glyph)`: `-advancementForCGGlyph:`
- `advancementForGlyph(glyph)`: `-advancementForGlyph:`
- `ascender()`: `-ascender`
- `boundingRectForCGGlyph(glyph)`: `-boundingRectForCGGlyph:`
- `boundingRectForFont()`: `-boundingRectForFont`
- `boundingRectForGlyph(glyph)`: `-boundingRectForGlyph:`
- `capHeight()`: `-capHeight`
- `coveredCharacterSet()`: `-coveredCharacterSet`
- `descender()`: `-descender`
- `displayName()`: `-displayName`
- `familyName()`: `-familyName`
- `fontDescriptor()`: `-fontDescriptor`
- `fontName()`: `-fontName`
- `fontWithSize(fontSize)`: `-fontWithSize:`
- `glyphWithName(name)`: `-glyphWithName:`
- `isFixedPitch()`: `-isFixedPitch`
- `isVertical()`: `-isVertical`
- `italicAngle()`: `-italicAngle`
- `leading()`: `-leading`
- `maximumAdvancement()`: `-maximumAdvancement`
- `mostCompatibleStringEncoding()`: `-mostCompatibleStringEncoding`
- `numberOfGlyphs()`: `-numberOfGlyphs`
- `pointSize()`: `-pointSize`
- `printerFont()`: `-printerFont`
- `renderingMode()`: `-renderingMode`
- `screenFont()`: `-screenFont`
- `screenFontWithRenderingMode(renderingMode)`: `-screenFontWithRenderingMode:`
- `textTransform()`: `-textTransform`
- `underlinePosition()`: `-underlinePosition`
- `underlineThickness()`: `-underlineThickness`
- `verticalFont()`: `-verticalFont`
- `xHeight()`: `-xHeight`

## Not bound

- `fontWithName:matrix:`: pointer
- `getAdvancements:forCGGlyphs:count:`: pointer
- `getAdvancements:forGlyphs:count:`: pointer
- `getAdvancements:forPackedGlyphs:length:`: pointer
- `getBoundingRects:forCGGlyphs:count:`: pointer
- `getBoundingRects:forGlyphs:count:`: pointer
- `matrix`: pointer
