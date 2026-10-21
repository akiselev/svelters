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
- `boundingRectForCGGlyph(glyph)`: `-boundingRectForCGGlyph:`
- `boundingRectForGlyph(glyph)`: `-boundingRectForGlyph:`
- `fontWithSize(fontSize)`: `-fontWithSize:`
- `glyphWithName(name)`: `-glyphWithName:`
- `printerFont()`: `-printerFont`
- `renderingMode()`: `-renderingMode`
- `screenFont()`: `-screenFont`
- `screenFontWithRenderingMode(renderingMode)`: `-screenFontWithRenderingMode:`

## Properties

- `ascender` (read-only): KVC `ascender` (probe)
- `boundingRectForFont` (read-only): KVC `boundingRectForFont` (probe)
- `capHeight` (read-only): KVC `capHeight` (probe)
- `coveredCharacterSet` (read-only): KVC `coveredCharacterSet` (probe)
- `descender` (read-only): KVC `descender` (probe)
- `displayName` (read-only): KVC `displayName` (probe)
- `familyName` (read-only): KVC `familyName` (probe)
- `fixedPitch` (read-only): KVC `fixedPitch` (probe)
- `fontDescriptor` (read-only): KVC `fontDescriptor` (probe)
- `fontName` (read-only): KVC `fontName` (probe)
- `italicAngle` (read-only): KVC `italicAngle` (probe)
- `leading` (read-only): KVC `leading` (probe)
- `maximumAdvancement` (read-only): KVC `maximumAdvancement` (probe)
- `mostCompatibleStringEncoding` (read-only): KVC `mostCompatibleStringEncoding` (probe)
- `numberOfGlyphs` (read-only): KVC `numberOfGlyphs` (probe)
- `pointSize` (read-only): KVC `pointSize` (probe)
- `textTransform` (read-only): KVC `textTransform` (probe)
- `underlinePosition` (read-only): KVC `underlinePosition` (probe)
- `underlineThickness` (read-only): KVC `underlineThickness` (probe)
- `vertical` (read-only): KVC `vertical` (probe)
- `verticalFont` (read-only): KVC `verticalFont` (probe)
- `xHeight` (read-only): KVC `xHeight` (probe)

## Not bound

- `fontWithName:matrix:`: pointer
- `getAdvancements:forCGGlyphs:count:`: pointer
- `getAdvancements:forGlyphs:count:`: pointer
- `getAdvancements:forPackedGlyphs:length:`: pointer
- `getBoundingRects:forCGGlyphs:count:`: pointer
- `getBoundingRects:forGlyphs:count:`: pointer
- `matrix`: pointer
