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
- `buildAttributeListForCopy(flags, cancellable)`, since 2.68
- `copyAttributes(destination, flags, cancellable)`
- `create(flags, cancellable)`
- `createReadwrite(flags, cancellable)`, since 2.22
- `delete(cancellable)`
- `dup()`
- `enumerateChildren(attributes, flags, cancellable)`
- `equal(file2)`
- `findEnclosingMount(cancellable)`
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
- `loadContents(cancellable)`
- `makeDirectory(cancellable)`
- `makeDirectoryWithParents(cancellable)`, since 2.18
- `makeSymbolicLink(symlinkValue, cancellable)`
- `measureDiskUsage(flags, cancellable, progressCallback)`, since 2.38; `progressCallback` is a callback (call scope)
- `monitor(flags, cancellable)`, since 2.18
- `monitorDirectory(flags, cancellable)`
- `monitorFile(flags, cancellable)`
- `move(destination, flags, cancellable, progressCallback)`; `progressCallback` is a callback (call scope)
- `openReadwrite(cancellable)`, since 2.22
- `peekPath()`, since 2.56
- `queryDefaultHandler(cancellable)`
- `queryExists(cancellable)`
- `queryFileType(flags, cancellable)`, since 2.18
- `queryFilesystemInfo(attributes, cancellable)`
- `queryInfo(attributes, flags, cancellable)`
- `querySettableAttributes(cancellable)`
- `queryWritableNamespaces(cancellable)`
- `read(cancellable)`
- `replace(etag, makeBackup, flags, cancellable)`
- `replaceContents(contents, etag, makeBackup, flags, cancellable)`
- `replaceReadwrite(etag, makeBackup, flags, cancellable)`, since 2.22
- `resolveRelativePath(relativePath)`
- `setAttributeByteString(attribute, value, flags, cancellable)`
- `setAttributeInt32(attribute, value, flags, cancellable)`
- `setAttributeInt64(attribute, value, flags, cancellable)`
- `setAttributeString(attribute, value, flags, cancellable)`
- `setAttributeUint32(attribute, value, flags, cancellable)`
- `setAttributeUint64(attribute, value, flags, cancellable)`
- `setAttributesFromInfo(info, flags, cancellable)`
- `setDisplayName(displayName, cancellable)`
- `supportsThreadContexts()`, since 2.22
- `trash(cancellable)`

## Async methods

Each folds a native `_async`/`_finish` pair into one call that returns a Promise. It resolves with the finish result (an object of the out parameters where there are any) and rejects with a `NativeError`. The native `GCancellable` is not exposed: pass `{ signal }` (an `AbortSignal`) as the last argument to cancel, which rejects with code `cancelled`. See [Async calls and callbacks](../async/).

- `appendToAsync(flags, ioPriority, { signal }?)` → `Promise<object>` (`append_to_async` + `append_to_finish`)
- `createAsync(flags, ioPriority, { signal }?)` → `Promise<object>` (`create_async` + `create_finish`)
- `createReadwriteAsync(flags, ioPriority, { signal }?)` → `Promise<object>` (`create_readwrite_async` + `create_readwrite_finish`), since 2.22
- `deleteAsync(ioPriority, { signal }?)` → `Promise<bool>` (`delete_async` + `delete_finish`), since 2.34
- `ejectMountableWithOperation(flags, mountOperation, { signal }?)` → `Promise<bool>` (`eject_mountable_with_operation` + `eject_mountable_with_operation_finish`), since 2.22
- `enumerateChildrenAsync(attributes, flags, ioPriority, { signal }?)` → `Promise<object>` (`enumerate_children_async` + `enumerate_children_finish`)
- `findEnclosingMountAsync(ioPriority, { signal }?)` → `Promise<object>` (`find_enclosing_mount_async` + `find_enclosing_mount_finish`)
- `loadBytesAsync({ signal }?)` → `Promise<{ ret, etagOut }>` (`load_bytes_async` + `load_bytes_finish`), since 2.56
- `loadContentsAsync({ signal }?)` → `Promise<{ ret, contents, etagOut }>` (`load_contents_async` + `load_contents_finish`)
- `makeDirectoryAsync(ioPriority, { signal }?)` → `Promise<bool>` (`make_directory_async` + `make_directory_finish`), since 2.38
- `makeSymbolicLinkAsync(symlinkValue, ioPriority, { signal }?)` → `Promise<bool>` (`make_symbolic_link_async` + `make_symbolic_link_finish`), since 2.74
- `mountEnclosingVolume(flags, mountOperation, { signal }?)` → `Promise<bool>` (`mount_enclosing_volume` + `mount_enclosing_volume_finish`)
- `mountMountable(flags, mountOperation, { signal }?)` → `Promise<object>` (`mount_mountable` + `mount_mountable_finish`)
- `openReadwriteAsync(ioPriority, { signal }?)` → `Promise<object>` (`open_readwrite_async` + `open_readwrite_finish`), since 2.22
- `pollMountable({ signal }?)` → `Promise<bool>` (`poll_mountable` + `poll_mountable_finish`), since 2.22
- `queryDefaultHandlerAsync(ioPriority, { signal }?)` → `Promise<object>` (`query_default_handler_async` + `query_default_handler_finish`), since 2.60
- `queryFilesystemInfoAsync(attributes, ioPriority, { signal }?)` → `Promise<object>` (`query_filesystem_info_async` + `query_filesystem_info_finish`)
- `queryInfoAsync(attributes, flags, ioPriority, { signal }?)` → `Promise<object>` (`query_info_async` + `query_info_finish`)
- `readAsync(ioPriority, { signal }?)` → `Promise<object>` (`read_async` + `read_finish`)
- `replaceAsync(etag, makeBackup, flags, ioPriority, { signal }?)` → `Promise<object>` (`replace_async` + `replace_finish`)
- `replaceContentsAsync(contents, etag, makeBackup, flags, { signal }?)` → `Promise<{ ret, newEtag }>` (`replace_contents_async` + `replace_contents_finish`)
- `replaceReadwriteAsync(etag, makeBackup, flags, ioPriority, { signal }?)` → `Promise<object>` (`replace_readwrite_async` + `replace_readwrite_finish`), since 2.22
- `setAttributesAsync(info, flags, ioPriority, { signal }?)` → `Promise<{ ret, info }>` (`set_attributes_async` + `set_attributes_finish`)
- `setDisplayNameAsync(displayName, ioPriority, { signal }?)` → `Promise<object>` (`set_display_name_async` + `set_display_name_finish`)
- `startMountable(flags, startOperation, { signal }?)` → `Promise<bool>` (`start_mountable` + `start_mountable_finish`), since 2.22
- `stopMountable(flags, mountOperation, { signal }?)` → `Promise<bool>` (`stop_mountable` + `stop_mountable_finish`), since 2.22
- `trashAsync(ioPriority, { signal }?)` → `Promise<bool>` (`trash_async` + `trash_finish`), since 2.38
- `unmountMountableWithOperation(flags, mountOperation, { signal }?)` → `Promise<bool>` (`unmount_mountable_with_operation` + `unmount_mountable_with_operation_finish`), since 2.22

## Not bound yet

4 members are not bound; reasons: async 3, gpointer 1.

