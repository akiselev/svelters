---
title: Gio.File
description: "Gio.File from svelters:gtk/gio, a shared object class generated from the GIR."
---

```js
import { File } from 'svelters:gtk/gio';
```

## Class

`Gio.File`, GType `GFile`.

Lifetime: a **SHARED** handle: one JS object per native object (`===` works), a strong reference held until the wrapper is collected or `dispose()`d. Dropping the wrapper never destroys the native object while anything else holds it.

Facts come from the GIR; the tests behind them are listed in the ledger (`metadata/verified/gtk.json`, fact ids `svelters-api:v1:type:gtk:Gio:File:*`).

## Statics

- `File.newBuildFilenamev(args)`, since 2.78
- `File.newForCommandlineArg(arg)`
- `File.newForCommandlineArgAndCwd(arg, cwd)`, since 2.36
- `File.newForPath(path)`
- `File.newForUri(uri)`
- `File.newTmp(tmpl)`, since 2.32
- `File.newTmpAsync(tmpl, ioPriority)`, since 2.74
- `File.newTmpDirAsync(tmpl, ioPriority)`, since 2.74
- `File.parseName(parseName)`

## Methods

- `appendTo(flags, cancellable)`
- `appendToAsync(flags, ioPriority)`
- `buildAttributeListForCopy(flags, cancellable)`, since 2.68
- `copyAttributes(destination, flags, cancellable)`
- `create(flags, cancellable)`
- `createAsync(flags, ioPriority)`
- `createReadwrite(flags, cancellable)`, since 2.22
- `createReadwriteAsync(flags, ioPriority)`, since 2.22
- `delete(cancellable)`
- `deleteAsync(ioPriority)`, since 2.34
- `dup()`
- `ejectMountableWithOperation(flags, mountOperation)`, since 2.22
- `enumerateChildren(attributes, flags, cancellable)`
- `enumerateChildrenAsync(attributes, flags, ioPriority)`
- `equal(file2)`
- `findEnclosingMount(cancellable)`
- `findEnclosingMountAsync(ioPriority)`
- `getBasename()`
- `getChild(name)`
- `getChildForDisplayName(displayName)`
- `getParent()`
- `getParseName()`
- `getPath()`
- `getRelativePath(descendant)`
- `getUri()`
- `getUriScheme()`
- `hasParent(parent)`, since 2.24
- `hasPrefix(prefix)`
- `hasUriScheme(uriScheme)`
- `hash()`
- `isNative()`
- `loadBytes(cancellable)`, since 2.56
- `loadBytesAsync()`, since 2.56
- `loadContents(cancellable)`
- `loadContentsAsync()`
- `makeDirectory(cancellable)`
- `makeDirectoryAsync(ioPriority)`, since 2.38
- `makeDirectoryWithParents(cancellable)`, since 2.18
- `makeSymbolicLink(symlinkValue, cancellable)`
- `makeSymbolicLinkAsync(symlinkValue, ioPriority)`, since 2.74
- `measureDiskUsage(flags, cancellable, progressCallback)`, since 2.38
- `monitor(flags, cancellable)`, since 2.18
- `monitorDirectory(flags, cancellable)`
- `monitorFile(flags, cancellable)`
- `mountEnclosingVolume(flags, mountOperation)`
- `mountMountable(flags, mountOperation)`
- `move(destination, flags, cancellable, progressCallback)`
- `openReadwrite(cancellable)`, since 2.22
- `openReadwriteAsync(ioPriority)`, since 2.22
- `peekPath()`, since 2.56
- `pollMountable()`, since 2.22
- `queryDefaultHandler(cancellable)`
- `queryDefaultHandlerAsync(ioPriority)`, since 2.60
- `queryExists(cancellable)`
- `queryFileType(flags, cancellable)`, since 2.18
- `queryFilesystemInfo(attributes, cancellable)`
- `queryFilesystemInfoAsync(attributes, ioPriority)`
- `queryInfo(attributes, flags, cancellable)`
- `queryInfoAsync(attributes, flags, ioPriority)`
- `querySettableAttributes(cancellable)`
- `queryWritableNamespaces(cancellable)`
- `read(cancellable)`
- `readAsync(ioPriority)`
- `replace(etag, makeBackup, flags, cancellable)`
- `replaceAsync(etag, makeBackup, flags, ioPriority)`
- `replaceContents(contents, etag, makeBackup, flags, cancellable)`
- `replaceContentsAsync(contents, etag, makeBackup, flags)`
- `replaceReadwrite(etag, makeBackup, flags, cancellable)`, since 2.22
- `replaceReadwriteAsync(etag, makeBackup, flags, ioPriority)`, since 2.22
- `resolveRelativePath(relativePath)`
- `setAttributeByteString(attribute, value, flags, cancellable)`
- `setAttributeInt32(attribute, value, flags, cancellable)`
- `setAttributeInt64(attribute, value, flags, cancellable)`
- `setAttributeString(attribute, value, flags, cancellable)`
- `setAttributeUint32(attribute, value, flags, cancellable)`
- `setAttributeUint64(attribute, value, flags, cancellable)`
- `setAttributesAsync(info, flags, ioPriority)`
- `setAttributesFromInfo(info, flags, cancellable)`
- `setDisplayName(displayName, cancellable)`
- `setDisplayNameAsync(displayName, ioPriority)`
- `startMountable(flags, startOperation)`, since 2.22
- `stopMountable(flags, mountOperation)`, since 2.22
- `supportsThreadContexts()`, since 2.22
- `trash(cancellable)`
- `trashAsync(ioPriority)`, since 2.38
- `unmountMountableWithOperation(flags, mountOperation)`, since 2.22

## Not bound yet

4 members are not bound; reasons: async 3, gpointer 1.

