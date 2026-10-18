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
- `monitor(flags, cancellable)`, since 2.18
- `monitorDirectory(flags, cancellable)`
- `monitorFile(flags, cancellable)`
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

## Not bound yet

36 members are not bound; reasons: async 32, callback 3, gpointer 1.

