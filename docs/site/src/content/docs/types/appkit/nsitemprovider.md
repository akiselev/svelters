---
title: NSItemProvider
description: "NSItemProvider from svelters:appkit/foundation, a object class generated from objc2 facts."
---

```js
import { NSItemProvider } from 'svelters:appkit/foundation';
```

## Class

`NSItemProvider` (Foundation), category **object**.

Lifetime: **SHARED**: one JS object per native object, holding one strong reference (retain); `dispose()` or garbage collection drops our reference only.

Facts: `metadata/appkit/types/Foundation.json`; ledger fact ids `svelters-api:v1:type:appkit:Foundation:NSItemProvider:*` in `metadata/verified/appkit.json`. Methods are called by selector; the JS name joins the selector's pieces.

## Construct

- `new NSItemProvider(props?)`: `[[NSItemProvider alloc] init]`, then the properties by KVC
- `NSItemProvider.newWithContentsOfURL(fileUrl)`: `-[NSItemProvider initWithContentsOfURL:]`
- `NSItemProvider.newWithItemTypeIdentifier(item, typeIdentifier)`: `-[NSItemProvider initWithItem:typeIdentifier:]`
- `NSItemProvider.newWithObject(object)`: `-[NSItemProvider initWithObject:]`

## Methods

- `containerFrame()`: `-containerFrame`
- `hasItemConformingToTypeIdentifier(typeIdentifier)`: `-hasItemConformingToTypeIdentifier:`
- `hasRepresentationConformingToTypeIdentifierFileOptions(typeIdentifier, fileOptions)`: `-hasRepresentationConformingToTypeIdentifier:fileOptions:`
- `loadDataRepresentationForTypeIdentifier(typeIdentifier)`: `-loadDataRepresentationForTypeIdentifier:completionHandler:` (async: returns a Promise; `{ signal }` cancels)
- `loadFileRepresentationForTypeIdentifier(typeIdentifier)`: `-loadFileRepresentationForTypeIdentifier:completionHandler:` (async: returns a Promise; `{ signal }` cancels)
- `preferredPresentationSize()`: `-preferredPresentationSize`
- `registerObjectVisibility(object, visibility)`: `-registerObject:visibility:`
- `registeredTypeIdentifiers()`: `-registeredTypeIdentifiers`
- `registeredTypeIdentifiersWithFileOptions(fileOptions)`: `-registeredTypeIdentifiersWithFileOptions:`
- `sourceFrame()`: `-sourceFrame`

## Properties

- `suggestedName`: KVC `suggestedName` (objc2)

## Not bound

- `loadInPlaceFileRepresentationForTypeIdentifier:completionHandler:`: block-results
- `loadItemForTypeIdentifier:options:completionHandler:`: type:unknown:NSItemProviderCompletionHandler
- `loadPreviewImageWithOptions:completionHandler:`: type:unknown:NSItemProviderCompletionHandler
- `previewImageHandler`: callback
- `registerCloudKitShare:container:`: type:unknown:CKShare
- `registerCloudKitShareWithPreparationHandler:`: callback
- `registerDataRepresentationForTypeIdentifier:visibility:loadHandler:`: callback
- `registerFileRepresentationForTypeIdentifier:fileOptions:visibility:loadHandler:`: callback
- `registerItemForTypeIdentifier:loadHandler:`: callback
