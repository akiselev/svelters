---
title: NSAttributedString
description: "NSAttributedString from svelters:appkit/foundation, a value class generated from objc2 facts."
---

```js
import { NSAttributedString } from 'svelters:appkit/foundation';
```

## Class

`NSAttributedString` (Foundation), category **value**.

Lifetime: an immutable Objective-C object behind a **SHARED** handle (one strong reference). Sharing an immutable object is indistinguishable from copying it, so there is no identity contract: `equals()` uses `isEqual:`, `toString()` is `description`. Setters and mutating selectors are hidden (decision Q3).

Facts: `metadata/appkit/types/Foundation.json`; ledger fact ids `svelters-api:v1:type:appkit:Foundation:NSAttributedString:*` in `metadata/verified/appkit.json`. Methods are called by selector; the JS name joins the selector's pieces.

## Construct

- `new NSAttributedString(string)`: `initWithString:`
- `new NSAttributedString(props?)`: `[[NSAttributedString alloc] init]`, then the properties by KVC
- `NSAttributedString.newWithAttributedString(attrStr)`: `-[NSAttributedString initWithAttributedString:]`
- `NSAttributedString.newWithContentsOfMarkdownFileAtURLOptionsBaseURL(markdownFile, options, baseUrl)`: `-[NSAttributedString initWithContentsOfMarkdownFileAtURL:options:baseURL:error:]`
- `NSAttributedString.newWithMarkdownOptionsBaseURL(markdown, options, baseUrl)`: `-[NSAttributedString initWithMarkdown:options:baseURL:error:]`
- `NSAttributedString.newWithMarkdownStringOptionsBaseURL(markdownString, options, baseUrl)`: `-[NSAttributedString initWithMarkdownString:options:baseURL:error:]`
- `NSAttributedString.newWithString(str)`: `-[NSAttributedString initWithString:]`
- `NSAttributedString.newWithStringAttributes(str, attrs)`: `-[NSAttributedString initWithString:attributes:]`

## Statics

- `NSAttributedString.attributedStringWithAdaptiveImageGlyphAttributes(adaptiveImageGlyph, attributes)`: `+attributedStringWithAdaptiveImageGlyph:attributes:`
- `NSAttributedString.attributedStringWithAttachment(attachment)`: `+attributedStringWithAttachment:`
- `NSAttributedString.attributedStringWithAttachmentAttributes(attachment, attributes)`: `+attributedStringWithAttachment:attributes:`
- `NSAttributedString.textTypes()`: `+textTypes`
- `NSAttributedString.textUnfilteredTypes()`: `+textUnfilteredTypes`

## Methods

- `RTFDFileWrapperFromRangeDocumentAttributes(range, dict)`: `-RTFDFileWrapperFromRange:documentAttributes:`
- `RTFDFromRangeDocumentAttributes(range, dict)`: `-RTFDFromRange:documentAttributes:`
- `RTFFromRangeDocumentAttributes(range, dict)`: `-RTFFromRange:documentAttributes:`
- `attributedStringByInflectingString()`: `-attributedStringByInflectingString`
- `attributedSubstringFromRange(range)`: `-attributedSubstringFromRange:`
- `boundingRectWithSizeOptions(size, options)`: `-boundingRectWithSize:options:`
- `boundingRectWithSizeOptionsContext(size, options, context)`: `-boundingRectWithSize:options:context:`
- `containsAttachments()`: `-containsAttachments`
- `containsAttachmentsInRange(range)`: `-containsAttachmentsInRange:`
- `dataFromRangeDocumentAttributes(range, dict)`: `-dataFromRange:documentAttributes:error:` (throws a NativeError)
- `docFormatFromRangeDocumentAttributes(range, dict)`: `-docFormatFromRange:documentAttributes:`
- `doubleClickAtIndex(location)`: `-doubleClickAtIndex:`
- `drawAtPoint(point)`: `-drawAtPoint:`
- `drawInRect(rect)`: `-drawInRect:`
- `drawWithRectOptions(rect, options)`: `-drawWithRect:options:`
- `drawWithRectOptionsContext(rect, options, context)`: `-drawWithRect:options:context:`
- `fileWrapperFromRangeDocumentAttributes(range, dict)`: `-fileWrapperFromRange:documentAttributes:error:` (throws a NativeError)
- `fontAttributesInRange(range)`: `-fontAttributesInRange:`
- `isEqualToAttributedString(other)`: `-isEqualToAttributedString:`
- `itemNumberInTextListAtIndex(list, location)`: `-itemNumberInTextList:atIndex:`
- `length()`: `-length`
- `lineBreakBeforeIndexWithinRange(location, aRange)`: `-lineBreakBeforeIndex:withinRange:`
- `lineBreakByHyphenatingBeforeIndexWithinRange(location, aRange)`: `-lineBreakByHyphenatingBeforeIndex:withinRange:`
- `nextWordFromIndexForward(location, isForward)`: `-nextWordFromIndex:forward:`
- `prefersRTFDInRange(range)`: `-prefersRTFDInRange:`
- `rangeOfTextBlockAtIndex(block, location)`: `-rangeOfTextBlock:atIndex:`
- `rangeOfTextListAtIndex(list, location)`: `-rangeOfTextList:atIndex:`
- `rangeOfTextTableAtIndex(table, location)`: `-rangeOfTextTable:atIndex:`
- `rulerAttributesInRange(range)`: `-rulerAttributesInRange:`
- `size()`: `-size`
- `string()`: `-string`

## Not bound

- `attribute:atIndex:effectiveRange:`: pointer
- `attribute:atIndex:longestEffectiveRange:inRange:`: pointer
- `attributesAtIndex:effectiveRange:`: pointer
- `attributesAtIndex:longestEffectiveRange:inRange:`: pointer
- `enumerateAttribute:inRange:options:usingBlock:`: callback
- `enumerateAttributesInRange:options:usingBlock:`: callback
- `initWithData:options:documentAttributes:error:`: pointer
- `initWithDocFormat:documentAttributes:`: pointer
- `initWithHTML:baseURL:documentAttributes:`: pointer
- `initWithHTML:documentAttributes:`: pointer
- `initWithHTML:options:documentAttributes:`: pointer
- `initWithRTF:documentAttributes:`: pointer
- `initWithRTFD:documentAttributes:`: pointer
- `initWithRTFDFileWrapper:documentAttributes:`: pointer
- `initWithURL:options:documentAttributes:error:`: pointer
