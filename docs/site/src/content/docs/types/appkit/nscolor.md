---
title: NSColor
description: "NSColor from svelters:appkit/appkit, a value class generated from objc2 facts."
---

```js
import { NSColor } from 'svelters:appkit/appkit';
```

## Class

`NSColor` (AppKit), category **value**.

Lifetime: an immutable Objective-C object behind a **SHARED** handle (one strong reference). Sharing an immutable object is indistinguishable from copying it, so there is no identity contract: `equals()` uses `isEqual:`, `toString()` is `description`. Setters and mutating selectors are hidden (decision Q3).

Facts: `metadata/appkit/types/AppKit.json`; ledger fact ids `svelters-api:v1:type:appkit:AppKit:NSColor:*` in `metadata/verified/appkit.json`. Methods are called by selector; the JS name joins the selector's pieces.

## Construct

- `new NSColor(props?)`: `[[NSColor alloc] init]`, then the properties by KVC

## Statics

- `NSColor.alternateSelectedControlTextColor()`: `+alternateSelectedControlTextColor`
- `NSColor.alternatingContentBackgroundColors()`: `+alternatingContentBackgroundColors`
- `NSColor.blackColor()`: `+blackColor`
- `NSColor.blueColor()`: `+blueColor`
- `NSColor.brownColor()`: `+brownColor`
- `NSColor.clearColor()`: `+clearColor`
- `NSColor.colorFromPasteboard(pasteBoard)`: `+colorFromPasteboard:`
- `NSColor.colorNamed(name)`: `+colorNamed:`
- `NSColor.colorNamedBundle(name, bundle)`: `+colorNamed:bundle:`
- `NSColor.colorWithCalibratedHueSaturationBrightnessAlpha(hue, saturation, brightness, alpha)`: `+colorWithCalibratedHue:saturation:brightness:alpha:`
- `NSColor.colorWithCalibratedRedGreenBlueAlpha(red, green, blue, alpha)`: `+colorWithCalibratedRed:green:blue:alpha:`
- `NSColor.colorWithCalibratedWhiteAlpha(white, alpha)`: `+colorWithCalibratedWhite:alpha:`
- `NSColor.colorWithCatalogNameColorName(listName, colorName)`: `+colorWithCatalogName:colorName:`
- `NSColor.colorWithColorSpaceHueSaturationBrightnessAlpha(space, hue, saturation, brightness, alpha)`: `+colorWithColorSpace:hue:saturation:brightness:alpha:`
- `NSColor.colorWithDeviceCyanMagentaYellowBlackAlpha(cyan, magenta, yellow, black, alpha)`: `+colorWithDeviceCyan:magenta:yellow:black:alpha:`
- `NSColor.colorWithDeviceHueSaturationBrightnessAlpha(hue, saturation, brightness, alpha)`: `+colorWithDeviceHue:saturation:brightness:alpha:`
- `NSColor.colorWithDeviceRedGreenBlueAlpha(red, green, blue, alpha)`: `+colorWithDeviceRed:green:blue:alpha:`
- `NSColor.colorWithDeviceWhiteAlpha(white, alpha)`: `+colorWithDeviceWhite:alpha:`
- `NSColor.colorWithDisplayP3RedGreenBlueAlpha(red, green, blue, alpha)`: `+colorWithDisplayP3Red:green:blue:alpha:`
- `NSColor.colorWithGenericGamma22WhiteAlpha(white, alpha)`: `+colorWithGenericGamma22White:alpha:`
- `NSColor.colorWithHueSaturationBrightnessAlpha(hue, saturation, brightness, alpha)`: `+colorWithHue:saturation:brightness:alpha:`
- `NSColor.colorWithPatternImage(image)`: `+colorWithPatternImage:`
- `NSColor.colorWithRedGreenBlueAlpha(red, green, blue, alpha)`: `+colorWithRed:green:blue:alpha:`
- `NSColor.colorWithRedGreenBlueAlphaExposure(red, green, blue, alpha, exposure)`: `+colorWithRed:green:blue:alpha:exposure:`
- `NSColor.colorWithRedGreenBlueAlphaLinearExposure(red, green, blue, alpha, linearExposure)`: `+colorWithRed:green:blue:alpha:linearExposure:`
- `NSColor.colorWithSRGBRedGreenBlueAlpha(red, green, blue, alpha)`: `+colorWithSRGBRed:green:blue:alpha:`
- `NSColor.colorWithWhiteAlpha(white, alpha)`: `+colorWithWhite:alpha:`
- `NSColor.controlAccentColor()`: `+controlAccentColor`
- `NSColor.controlBackgroundColor()`: `+controlBackgroundColor`
- `NSColor.controlColor()`: `+controlColor`
- `NSColor.controlTextColor()`: `+controlTextColor`
- `NSColor.currentControlTint()`: `+currentControlTint`
- `NSColor.cyanColor()`: `+cyanColor`
- `NSColor.darkGrayColor()`: `+darkGrayColor`
- `NSColor.disabledControlTextColor()`: `+disabledControlTextColor`
- `NSColor.findHighlightColor()`: `+findHighlightColor`
- `NSColor.grayColor()`: `+grayColor`
- `NSColor.greenColor()`: `+greenColor`
- `NSColor.gridColor()`: `+gridColor`
- `NSColor.headerTextColor()`: `+headerTextColor`
- `NSColor.highlightColor()`: `+highlightColor`
- `NSColor.keyboardFocusIndicatorColor()`: `+keyboardFocusIndicatorColor`
- `NSColor.labelColor()`: `+labelColor`
- `NSColor.lightGrayColor()`: `+lightGrayColor`
- `NSColor.linkColor()`: `+linkColor`
- `NSColor.magentaColor()`: `+magentaColor`
- `NSColor.orangeColor()`: `+orangeColor`
- `NSColor.placeholderTextColor()`: `+placeholderTextColor`
- `NSColor.purpleColor()`: `+purpleColor`
- `NSColor.quaternaryLabelColor()`: `+quaternaryLabelColor`
- `NSColor.quaternarySystemFillColor()`: `+quaternarySystemFillColor`
- `NSColor.quinaryLabelColor()`: `+quinaryLabelColor`
- `NSColor.quinarySystemFillColor()`: `+quinarySystemFillColor`
- `NSColor.redColor()`: `+redColor`
- `NSColor.scrubberTexturedBackgroundColor()`: `+scrubberTexturedBackgroundColor`
- `NSColor.secondaryLabelColor()`: `+secondaryLabelColor`
- `NSColor.secondarySystemFillColor()`: `+secondarySystemFillColor`
- `NSColor.selectedContentBackgroundColor()`: `+selectedContentBackgroundColor`
- `NSColor.selectedControlColor()`: `+selectedControlColor`
- `NSColor.selectedControlTextColor()`: `+selectedControlTextColor`
- `NSColor.selectedMenuItemTextColor()`: `+selectedMenuItemTextColor`
- `NSColor.selectedTextBackgroundColor()`: `+selectedTextBackgroundColor`
- `NSColor.selectedTextColor()`: `+selectedTextColor`
- `NSColor.separatorColor()`: `+separatorColor`
- `NSColor.shadowColor()`: `+shadowColor`
- `NSColor.systemBlueColor()`: `+systemBlueColor`
- `NSColor.systemBrownColor()`: `+systemBrownColor`
- `NSColor.systemCyanColor()`: `+systemCyanColor`
- `NSColor.systemFillColor()`: `+systemFillColor`
- `NSColor.systemGrayColor()`: `+systemGrayColor`
- `NSColor.systemGreenColor()`: `+systemGreenColor`
- `NSColor.systemIndigoColor()`: `+systemIndigoColor`
- `NSColor.systemMintColor()`: `+systemMintColor`
- `NSColor.systemOrangeColor()`: `+systemOrangeColor`
- `NSColor.systemPinkColor()`: `+systemPinkColor`
- `NSColor.systemPurpleColor()`: `+systemPurpleColor`
- `NSColor.systemRedColor()`: `+systemRedColor`
- `NSColor.systemTealColor()`: `+systemTealColor`
- `NSColor.systemYellowColor()`: `+systemYellowColor`
- `NSColor.tertiaryLabelColor()`: `+tertiaryLabelColor`
- `NSColor.tertiarySystemFillColor()`: `+tertiarySystemFillColor`
- `NSColor.textBackgroundColor()`: `+textBackgroundColor`
- `NSColor.textColor()`: `+textColor`
- `NSColor.textInsertionPointColor()`: `+textInsertionPointColor`
- `NSColor.underPageBackgroundColor()`: `+underPageBackgroundColor`
- `NSColor.unemphasizedSelectedContentBackgroundColor()`: `+unemphasizedSelectedContentBackgroundColor`
- `NSColor.unemphasizedSelectedTextBackgroundColor()`: `+unemphasizedSelectedTextBackgroundColor`
- `NSColor.unemphasizedSelectedTextColor()`: `+unemphasizedSelectedTextColor`
- `NSColor.whiteColor()`: `+whiteColor`
- `NSColor.windowBackgroundColor()`: `+windowBackgroundColor`
- `NSColor.windowFrameTextColor()`: `+windowFrameTextColor`
- `NSColor.yellowColor()`: `+yellowColor`

## Methods

- `alphaComponent()`: `-alphaComponent`
- `blackComponent()`: `-blackComponent`
- `blendedColorWithFractionOfColor(fraction, color)`: `-blendedColorWithFraction:ofColor:`
- `blueComponent()`: `-blueComponent`
- `brightnessComponent()`: `-brightnessComponent`
- `catalogNameComponent()`: `-catalogNameComponent`
- `colorByApplyingContentHeadroom(contentHeadroom)`: `-colorByApplyingContentHeadroom:`
- `colorNameComponent()`: `-colorNameComponent`
- `colorSpace()`: `-colorSpace`
- `colorUsingColorSpace(space)`: `-colorUsingColorSpace:`
- `colorUsingType(type)`: `-colorUsingType:`
- `colorWithAlphaComponent(alpha)`: `-colorWithAlphaComponent:`
- `colorWithSystemEffect(systemEffect)`: `-colorWithSystemEffect:`
- `cyanComponent()`: `-cyanComponent`
- `drawSwatchInRect(rect)`: `-drawSwatchInRect:`
- `greenComponent()`: `-greenComponent`
- `highlightWithLevel(val)`: `-highlightWithLevel:`
- `hueComponent()`: `-hueComponent`
- `linearExposure()`: `-linearExposure`
- `localizedCatalogNameComponent()`: `-localizedCatalogNameComponent`
- `localizedColorNameComponent()`: `-localizedColorNameComponent`
- `magentaComponent()`: `-magentaComponent`
- `numberOfComponents()`: `-numberOfComponents`
- `patternImage()`: `-patternImage`
- `redComponent()`: `-redComponent`
- `saturationComponent()`: `-saturationComponent`
- `shadowWithLevel(val)`: `-shadowWithLevel:`
- `standardDynamicRangeColor()`: `-standardDynamicRangeColor`
- `type(type)`: `-type`
- `whiteComponent()`: `-whiteComponent`
- `writeToPasteboard(pasteBoard)`: `-writeToPasteboard:`
- `yellowComponent()`: `-yellowComponent`

## Not bound

- `CGColor`: type:unknown:CGColor
- `colorWithCGColor:`: type:unknown:CGColor
- `colorWithCIColor:`: type:unknown:CIColor
- `colorWithColorSpace:components:count:`: pointer
- `colorWithName:dynamicProvider:`: callback
- `getComponents:`: pointer
- `getCyan:magenta:yellow:black:alpha:`: pointer
- `getHue:saturation:brightness:alpha:`: pointer
- `getRed:green:blue:alpha:`: pointer
- `getWhite:alpha:`: pointer
- `initWithCoder:`: type:excluded:NSCoder
