---
title: NSFileManager
description: "NSFileManager from svelters:appkit/foundation, a object class generated from objc2 facts."
---

```js
import { NSFileManager } from 'svelters:appkit/foundation';
```

## Class

`NSFileManager` (Foundation), category **object**.

Lifetime: **SHARED**: one JS object per native object, holding one strong reference (retain); `dispose()` or garbage collection drops our reference only.

Facts: `metadata/appkit/types/Foundation.json`; ledger fact ids `svelters-api:v1:type:appkit:Foundation:NSFileManager:*` in `metadata/verified/appkit.json`. Methods are called by selector; the JS name joins the selector's pieces.

## Construct

- `new NSFileManager(props?)`: `[[NSFileManager alloc] init]`, then the properties by KVC

## Statics

- `NSFileManager.defaultManager()`: `+defaultManager`
- `NSFileManager.fileManagerWithAuthorization(authorization)`: `+fileManagerWithAuthorization:`

## Methods

- `URLForDirectoryInDomainAppropriateForURLCreate(directory, domain, url, shouldCreate)`: `-URLForDirectory:inDomain:appropriateForURL:create:error:` (throws a NativeError)
- `URLForUbiquityContainerIdentifier(containerIdentifier)`: `-URLForUbiquityContainerIdentifier:`
- `URLsForDirectoryInDomains(directory, domainMask)`: `-URLsForDirectory:inDomains:`
- `attributesOfFileSystemForPath(path)`: `-attributesOfFileSystemForPath:error:` (throws a NativeError)
- `attributesOfItemAtPath(path)`: `-attributesOfItemAtPath:error:` (throws a NativeError)
- `changeCurrentDirectoryPath(path)`: `-changeCurrentDirectoryPath:`
- `componentsToDisplayForPath(path)`: `-componentsToDisplayForPath:`
- `containerURLForSecurityApplicationGroupIdentifier(groupIdentifier)`: `-containerURLForSecurityApplicationGroupIdentifier:`
- `contentsAtPath(path)`: `-contentsAtPath:`
- `contentsEqualAtPathAndPath(path1, path2)`: `-contentsEqualAtPath:andPath:`
- `contentsOfDirectoryAtPath(path)`: `-contentsOfDirectoryAtPath:error:` (throws a NativeError)
- `contentsOfDirectoryAtURLIncludingPropertiesForKeysOptions(url, keys, mask)`: `-contentsOfDirectoryAtURL:includingPropertiesForKeys:options:error:` (throws a NativeError)
- `copyItemAtPathToPath(srcPath, dstPath)`: `-copyItemAtPath:toPath:error:` (throws a NativeError)
- `copyItemAtURLToURL(srcUrl, dstUrl)`: `-copyItemAtURL:toURL:error:` (throws a NativeError)
- `createDirectoryAtPathWithIntermediateDirectoriesAttributes(path, createIntermediates, attributes)`: `-createDirectoryAtPath:withIntermediateDirectories:attributes:error:` (throws a NativeError)
- `createDirectoryAtURLWithIntermediateDirectoriesAttributes(url, createIntermediates, attributes)`: `-createDirectoryAtURL:withIntermediateDirectories:attributes:error:` (throws a NativeError)
- `createFileAtPathContentsAttributes(path, data, attr)`: `-createFileAtPath:contents:attributes:`
- `createSymbolicLinkAtPathWithDestinationPath(path, destPath)`: `-createSymbolicLinkAtPath:withDestinationPath:error:` (throws a NativeError)
- `createSymbolicLinkAtURLWithDestinationURL(url, destUrl)`: `-createSymbolicLinkAtURL:withDestinationURL:error:` (throws a NativeError)
- `currentDirectoryPath()`: `-currentDirectoryPath`
- `destinationOfSymbolicLinkAtPath(path)`: `-destinationOfSymbolicLinkAtPath:error:` (throws a NativeError)
- `displayNameAtPath(path)`: `-displayNameAtPath:`
- `evictUbiquitousItemAtURL(url)`: `-evictUbiquitousItemAtURL:error:` (throws a NativeError)
- `fetchLatestRemoteVersionOfItemAtURL(url)`: `-fetchLatestRemoteVersionOfItemAtURL:completionHandler:` (async: returns a Promise; `{ signal }` cancels)
- `fileExistsAtPath(path)`: `-fileExistsAtPath:`
- `getFileProviderServicesForItemAtURL(url)`: `-getFileProviderServicesForItemAtURL:completionHandler:` (async: returns a Promise; `{ signal }` cancels)
- `homeDirectoryForCurrentUser()`: `-homeDirectoryForCurrentUser`
- `homeDirectoryForUser(userName)`: `-homeDirectoryForUser:`
- `isDeletableFileAtPath(path)`: `-isDeletableFileAtPath:`
- `isExecutableFileAtPath(path)`: `-isExecutableFileAtPath:`
- `isReadableFileAtPath(path)`: `-isReadableFileAtPath:`
- `isUbiquitousItemAtURL(url)`: `-isUbiquitousItemAtURL:`
- `isWritableFileAtPath(path)`: `-isWritableFileAtPath:`
- `linkItemAtPathToPath(srcPath, dstPath)`: `-linkItemAtPath:toPath:error:` (throws a NativeError)
- `linkItemAtURLToURL(srcUrl, dstUrl)`: `-linkItemAtURL:toURL:error:` (throws a NativeError)
- `mountedVolumeURLsIncludingResourceValuesForKeysOptions(propertyKeys, options)`: `-mountedVolumeURLsIncludingResourceValuesForKeys:options:`
- `moveItemAtPathToPath(srcPath, dstPath)`: `-moveItemAtPath:toPath:error:` (throws a NativeError)
- `moveItemAtURLToURL(srcUrl, dstUrl)`: `-moveItemAtURL:toURL:error:` (throws a NativeError)
- `pauseSyncForUbiquitousItemAtURL(url)`: `-pauseSyncForUbiquitousItemAtURL:completionHandler:` (async: returns a Promise; `{ signal }` cancels)
- `removeItemAtPath(path)`: `-removeItemAtPath:error:` (throws a NativeError)
- `removeItemAtURL(url)`: `-removeItemAtURL:error:` (throws a NativeError)
- `resumeSyncForUbiquitousItemAtURLWithBehavior(url, behavior)`: `-resumeSyncForUbiquitousItemAtURL:withBehavior:completionHandler:` (async: returns a Promise; `{ signal }` cancels)
- `setAttributesOfItemAtPath(attributes, path)`: `-setAttributes:ofItemAtPath:error:` (throws a NativeError)
- `setUbiquitousItemAtURLDestinationURL(flag, url, destinationUrl)`: `-setUbiquitous:itemAtURL:destinationURL:error:` (throws a NativeError)
- `startDownloadingUbiquitousItemAtURL(url)`: `-startDownloadingUbiquitousItemAtURL:error:` (throws a NativeError)
- `subpathsAtPath(path)`: `-subpathsAtPath:`
- `subpathsOfDirectoryAtPath(path)`: `-subpathsOfDirectoryAtPath:error:` (throws a NativeError)
- `temporaryDirectory()`: `-temporaryDirectory`
- `unmountVolumeAtURLOptions(url, mask)`: `-unmountVolumeAtURL:options:completionHandler:` (async: returns a Promise; `{ signal }` cancels)
- `uploadLocalVersionOfUbiquitousItemAtURLWithConflictResolutionPolicy(url, conflictResolutionPolicy)`: `-uploadLocalVersionOfUbiquitousItemAtURL:withConflictResolutionPolicy:completionHandler:` (async: returns a Promise; `{ signal }` cancels)

## Not bound

- `URLForPublishingUbiquitousItemAtURL:expirationDate:error:`: pointer
- `enumeratorAtPath:`: type:excluded:NSDirectoryEnumerator
- `enumeratorAtURL:includingPropertiesForKeys:options:errorHandler:`: block-returns
- `fileExistsAtPath:isDirectory:`: pointer
- `fileSystemRepresentationWithPath:`: pointer
- `getRelationship:ofDirectory:inDomain:toItemAtURL:error:`: pointer
- `getRelationship:ofDirectoryAtURL:toItemAtURL:error:`: pointer
- `replaceItemAtURL:withItemAtURL:backupItemName:options:resultingItemURL:error:`: pointer
- `stringWithFileSystemRepresentation:length:`: pointer
- `trashItemAtURL:resultingItemURL:error:`: pointer
- `ubiquityIdentityToken`: type:unknown:AnyObject /* NSObjectProtocol+ NSCopying+ NSCoding */
