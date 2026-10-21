---
title: NSURL
description: "NSURL from svelters:appkit/foundation, a value class generated from objc2 facts."
---

```js
import { NSURL } from 'svelters:appkit/foundation';
```

## Class

`NSURL` (Foundation), category **value**.

Lifetime: an immutable Objective-C object behind a **SHARED** handle (one strong reference). Sharing an immutable object is indistinguishable from copying it, so there is no identity contract: `equals()` uses `isEqual:`, `toString()` is `description`. Setters and mutating selectors are hidden (decision Q3).

Facts: `metadata/appkit/types/Foundation.json`; ledger fact ids `svelters-api:v1:type:appkit:Foundation:NSURL:*` in `metadata/verified/appkit.json`. Methods are called by selector; the JS name joins the selector's pieces.

## Construct

- `new NSURL(string)`: `initWithString:`
- `new NSURL(props?)`: `[[NSURL alloc] init]`, then the properties by KVC
- `NSURL.newAbsoluteURLWithDataRepresentationRelativeToURL(data, baseUrl)`: `-[NSURL initAbsoluteURLWithDataRepresentation:relativeToURL:]`
- `NSURL.newFileURLWithPath(path)`: `-[NSURL initFileURLWithPath:]`
- `NSURL.newFileURLWithPathIsDirectory(path, isDir)`: `-[NSURL initFileURLWithPath:isDirectory:]`
- `NSURL.newFileURLWithPathIsDirectoryRelativeToURL(path, isDir, baseUrl)`: `-[NSURL initFileURLWithPath:isDirectory:relativeToURL:]`
- `NSURL.newFileURLWithPathRelativeToURL(path, baseUrl)`: `-[NSURL initFileURLWithPath:relativeToURL:]`
- `NSURL.newWithDataRepresentationRelativeToURL(data, baseUrl)`: `-[NSURL initWithDataRepresentation:relativeToURL:]`
- `NSURL.newWithString(urlString)`: `-[NSURL initWithString:]`
- `NSURL.newWithStringEncodingInvalidCharacters(urlString, encodingInvalidCharacters)`: `-[NSURL initWithString:encodingInvalidCharacters:]`
- `NSURL.newWithStringRelativeToURL(urlString, baseUrl)`: `-[NSURL initWithString:relativeToURL:]`

## Statics

- `NSURL.URLByResolvingAliasFileAtURLOptions(url, options)`: `+URLByResolvingAliasFileAtURL:options:error:` (throws)
- `NSURL.URLFromPasteboard(pasteBoard)`: `+URLFromPasteboard:`
- `NSURL.URLWithDataRepresentationRelativeToURL(data, baseUrl)`: `+URLWithDataRepresentation:relativeToURL:`
- `NSURL.URLWithString(urlString)`: `+URLWithString:`
- `NSURL.URLWithStringEncodingInvalidCharacters(urlString, encodingInvalidCharacters)`: `+URLWithString:encodingInvalidCharacters:`
- `NSURL.URLWithStringRelativeToURL(urlString, baseUrl)`: `+URLWithString:relativeToURL:`
- `NSURL.absoluteURLWithDataRepresentationRelativeToURL(data, baseUrl)`: `+absoluteURLWithDataRepresentation:relativeToURL:`
- `NSURL.bookmarkDataWithContentsOfURL(bookmarkFileUrl)`: `+bookmarkDataWithContentsOfURL:error:` (throws)
- `NSURL.fileURLWithPath(path)`: `+fileURLWithPath:`
- `NSURL.fileURLWithPathComponents(components)`: `+fileURLWithPathComponents:`
- `NSURL.fileURLWithPathIsDirectory(path, isDir)`: `+fileURLWithPath:isDirectory:`
- `NSURL.fileURLWithPathIsDirectoryRelativeToURL(path, isDir, baseUrl)`: `+fileURLWithPath:isDirectory:relativeToURL:`
- `NSURL.fileURLWithPathRelativeToURL(path, baseUrl)`: `+fileURLWithPath:relativeToURL:`
- `NSURL.resourceValuesForKeysFromBookmarkData(keys, bookmarkData)`: `+resourceValuesForKeys:fromBookmarkData:`
- `NSURL.writeBookmarkDataToURLOptions(bookmarkData, bookmarkFileUrl, options)`: `+writeBookmarkData:toURL:options:error:` (throws)

## Methods

- `URLByAppendingPathComponent(pathComponent)`: `-URLByAppendingPathComponent:`
- `URLByAppendingPathComponentIsDirectory(pathComponent, isDirectory)`: `-URLByAppendingPathComponent:isDirectory:`
- `URLByAppendingPathExtension(pathExtension)`: `-URLByAppendingPathExtension:`
- `URLByDeletingLastPathComponent()`: `-URLByDeletingLastPathComponent`
- `URLByDeletingPathExtension()`: `-URLByDeletingPathExtension`
- `URLByResolvingSymlinksInPath()`: `-URLByResolvingSymlinksInPath`
- `URLByStandardizingPath()`: `-URLByStandardizingPath`
- `absoluteString()`: `-absoluteString`
- `absoluteURL()`: `-absoluteURL`
- `baseURL()`: `-baseURL`
- `bookmarkDataWithOptionsIncludingResourceValuesForKeysRelativeToURL(options, keys, relativeUrl)`: `-bookmarkDataWithOptions:includingResourceValuesForKeys:relativeToURL:error:` (throws a NativeError)
- `checkPromisedItemIsReachableAndReturnError()`: `-checkPromisedItemIsReachableAndReturnError:` (throws a NativeError)
- `checkResourceIsReachableAndReturnError()`: `-checkResourceIsReachableAndReturnError:` (throws a NativeError)
- `dataRepresentation()`: `-dataRepresentation`
- `filePathURL()`: `-filePathURL`
- `fileReferenceURL()`: `-fileReferenceURL`
- `fragment()`: `-fragment`
- `hasDirectoryPath()`: `-hasDirectoryPath`
- `host()`: `-host`
- `isFileReferenceURL()`: `-isFileReferenceURL`
- `isFileURL()`: `-isFileURL`
- `lastPathComponent()`: `-lastPathComponent`
- `password()`: `-password`
- `path()`: `-path`
- `pathComponents()`: `-pathComponents`
- `pathExtension()`: `-pathExtension`
- `port()`: `-port`
- `promisedItemResourceValuesForKeys(keys)`: `-promisedItemResourceValuesForKeys:error:` (throws a NativeError)
- `query()`: `-query`
- `relativePath()`: `-relativePath`
- `relativeString()`: `-relativeString`
- `resourceSpecifier()`: `-resourceSpecifier`
- `resourceValuesForKeys(keys)`: `-resourceValuesForKeys:error:` (throws a NativeError)
- `scheme()`: `-scheme`
- `standardizedURL()`: `-standardizedURL`
- `startAccessingSecurityScopedResource()`: `-startAccessingSecurityScopedResource`
- `stopAccessingSecurityScopedResource()`: `-stopAccessingSecurityScopedResource`
- `user()`: `-user`
- `writeToPasteboard(pasteBoard)`: `-writeToPasteboard:`

## Not bound

- `URLByResolvingBookmarkData:options:relativeToURL:bookmarkDataIsStale:error:`: pointer
- `fileSystemRepresentation`: pointer
- `fileURLWithFileSystemRepresentation:isDirectory:relativeToURL:`: pointer
- `getFileSystemRepresentation:maxLength:`: pointer
- `getPromisedItemResourceValue:forKey:error:`: pointer
- `getResourceValue:forKey:error:`: pointer
- `initByResolvingBookmarkData:options:relativeToURL:bookmarkDataIsStale:error:`: pointer
- `initFileURLWithFileSystemRepresentation:isDirectory:relativeToURL:`: pointer
