---
title: StorageFolder
description: "Windows.Storage.StorageFolder from svelters:winui/windows.storage, a object class generated from winmd facts."
---

```js
import { StorageFolder } from 'svelters:winui/windows.storage';
```

## Class

`Windows.Storage.StorageFolder`, category **object**, thread class **any**.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), holding one COM reference; `dispose()` or the wrapper's garbage collection releases it (never `Close()`: that is an explicit method).

Facts: `metadata/winui/types/Windows.Storage.json`; ledger fact ids `svelters-api:v1:type:winui:Windows.Storage:StorageFolder:*` in `metadata/verified/winui.json`.

## Construct

Not constructible from JS.

## Statics

- `StorageFolder.getFolderFromPathAsync(path)` (async: a Promise; `{ signal }` cancels)

## Properties

- `attributes` (flags, read-only)
- `dateCreated` (datetime, read-only)
- `displayName` (string, read-only)
- `displayType` (string, read-only)
- `folderRelativeId` (string, read-only)
- `name` (string, read-only)
- `path` (string, read-only)
- `properties` (object, read-only)
- `provider` (object, read-only)

## Methods

- `createFileAsync(desiredName)` (async: a Promise; `{ signal }` cancels)
- `createFileAsync(desiredName, options)` (async: a Promise; `{ signal }` cancels)
- `createFolderAsync(desiredName)` (async: a Promise; `{ signal }` cancels)
- `createFolderAsync(desiredName, options)` (async: a Promise; `{ signal }` cancels)
- `deleteAsync()` (async: a Promise; `{ signal }` cancels)
- `deleteAsync(option)` (async: a Promise; `{ signal }` cancels)
- `getBasicPropertiesAsync()` (async: a Promise; `{ signal }` cancels)
- `getFileAsync(name)` (async: a Promise; `{ signal }` cancels)
- `getFilesAsync()` (async: a Promise; `{ signal }` cancels)
- `getFilesAsync(query, startIndex, maxItemsToRetrieve)` (async: a Promise; `{ signal }` cancels)
- `getFilesAsync(query)` (async: a Promise; `{ signal }` cancels)
- `getFolderAsync(name)` (async: a Promise; `{ signal }` cancels)
- `getFoldersAsync()` (async: a Promise; `{ signal }` cancels)
- `getFoldersAsync(query, startIndex, maxItemsToRetrieve)` (async: a Promise; `{ signal }` cancels)
- `getFoldersAsync(query)` (async: a Promise; `{ signal }` cancels)
- `getIndexedStateAsync()` (async: a Promise; `{ signal }` cancels)
- `getItemAsync(name)` (async: a Promise; `{ signal }` cancels)
- `getItemsAsync()` (async: a Promise; `{ signal }` cancels)
- `getItemsAsync(startIndex, maxItemsToRetrieve)` (async: a Promise; `{ signal }` cancels)
- `getParentAsync()` (async: a Promise; `{ signal }` cancels)
- `getScaledImageAsThumbnailAsync(mode)` (async: a Promise; `{ signal }` cancels)
- `getScaledImageAsThumbnailAsync(mode, requestedSize)` (async: a Promise; `{ signal }` cancels)
- `getScaledImageAsThumbnailAsync(mode, requestedSize, options)` (async: a Promise; `{ signal }` cancels)
- `getThumbnailAsync(mode)` (async: a Promise; `{ signal }` cancels)
- `getThumbnailAsync(mode, requestedSize)` (async: a Promise; `{ signal }` cancels)
- `getThumbnailAsync(mode, requestedSize, options)` (async: a Promise; `{ signal }` cancels)
- `isCommonFileQuerySupported(query)`
- `isCommonFolderQuerySupported(query)`
- `isEqual(item)`
- `isOfType(type)`
- `renameAsync(desiredName)` (async: a Promise; `{ signal }` cancels)
- `renameAsync(desiredName, option)` (async: a Promise; `{ signal }` cancels)
- `tryGetChangeTracker()`
- `tryGetItemAsync(name)` (async: a Promise; `{ signal }` cancels)

## Not bound

- `AreQueryOptionsSupported/1`: out-of-scope:Windows.Storage.Search.QueryOptions
- `CreateFileQuery/0`: out-of-scope:Windows.Storage.Search.StorageFileQueryResult
- `CreateFileQuery/1`: out-of-scope:Windows.Storage.Search.StorageFileQueryResult
- `CreateFileQueryWithOptions/1`: out-of-scope:Windows.Storage.Search.QueryOptions
- `CreateFolderQuery/0`: out-of-scope:Windows.Storage.Search.StorageFolderQueryResult
- `CreateFolderQuery/1`: out-of-scope:Windows.Storage.Search.StorageFolderQueryResult
- `CreateFolderQueryWithOptions/1`: out-of-scope:Windows.Storage.Search.QueryOptions
- `CreateItemQuery/0`: out-of-scope:Windows.Storage.Search.StorageItemQueryResult
- `CreateItemQueryWithOptions/1`: out-of-scope:Windows.Storage.Search.QueryOptions
- `static:GetFolderFromPathForUserAsync/2`: out-of-scope:Windows.System.User
