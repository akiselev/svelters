---
title: Non-UI types (WinUI)
description: "Windows Runtime classes as ES module named exports (svelters:winui/windows.foundation, ...)."
---

Every bound Windows Runtime class is a named export of `svelters:winui/<namespace>` (the namespace in lower case). WinUI controls stay elements (tags).

```svelte
<script>
  import { Uri } from 'svelters:winui/windows.foundation';
  import { StorageFolder, FileIO } from 'svelters:winui/windows.storage';
  const uri = new Uri('https://example.com/a?x=1');
  const folder = await StorageFolder.getFolderFromPathAsync(dir);
  const file = await folder.createFileAsync('note.txt', 'replace-existing');
  await FileIO.writeTextAsync(file, 'hello');
</script>
<text-block text={uri.host} />
```

- **Objects**: one JS object per native object, a SHARED handle (one COM reference); `dispose()` or garbage collection releases it. `Close()` (IClosable) is an explicit method.
- **Records** (`Color`, `Thickness`, `CornerRadius`, `Point`): plain objects with camelCase field names. `DateTime` is a `Date`, `TimeSpan` milliseconds.
- **Enums**: kebab-case names in and out (native and camelCase names and numbers are accepted); flags are lists of names.
- **Async** (`IAsyncOperation`, `IAsyncAction`): Promises; a trailing `{ signal }` cancels (`IAsyncInfo.Cancel`); a failure rejects with `NativeError { domain: 'winrt', code }` (`file-not-found`, `access-denied`, ...).
- **Collections**: `IVectorView`/`IIterable` results are arrays, `IMapView` plain objects; `IVector`/`IMap` results are live objects (`getAt`, `size`, `append`, `insert`, `lookup`, ...).
- **Overloads**: one JS name; the backend picks the overload by argument count.
- **Threads**: one UI thread; completions hop to it before anything is converted.

## Curated types

- [Microsoft.UI.ColorHelper](colorhelper/)
- [Microsoft.UI.Dispatching.DispatcherQueueTimer](dispatcherqueuetimer/)
- [Microsoft.UI.Xaml.CornerRadiusHelper](cornerradiushelper/)
- [Microsoft.UI.Xaml.Media.FontFamily](fontfamily/)
- [Microsoft.UI.Xaml.Media.SolidColorBrush](solidcolorbrush/)
- [Microsoft.UI.Xaml.ThicknessHelper](thicknesshelper/)
- [Windows.ApplicationModel.DataTransfer.DataPackage](datapackage/)
- [Windows.Data.Json.JsonObject](jsonobject/)
- [Windows.Foundation.Collections.PropertySet](propertyset/)
- [Windows.Foundation.Uri](uri/)
- [Windows.Storage.FileIO](fileio/)
- [Windows.Storage.StorageFolder](storagefolder/)

## All bound types

- `svelters:winui/microsoft.ui`: `ColorHelper` (statics), `Colors` (statics)
- `svelters:winui/microsoft.ui.composition`: `Compositor` (object)
- `svelters:winui/microsoft.ui.dispatching`: `DispatcherQueue` (object), `DispatcherQueueShutdownStartingEventArgs` (args), `DispatcherQueueTimer` (object)
- `svelters:winui/microsoft.ui.xaml`: `CornerRadiusHelper` (statics), `DependencyObject` (object), `DependencyProperty` (object), `ExceptionRoutedEventArgs` (args), `GridLengthHelper` (statics), `RoutedEventArgs` (args), `ThicknessHelper` (statics)
- `svelters:winui/microsoft.ui.xaml.media`: `Brush` (object), `FontFamily` (object), `GeneralTransform` (object), `GradientBrush` (object), `GradientStop` (object), `GradientStopCollection` (model), `ImageSource` (object), `LinearGradientBrush` (object), `SolidColorBrush` (object), `Transform` (object)
- `svelters:winui/microsoft.ui.xaml.media.imaging`: `BitmapImage` (object), `BitmapSource` (object), `DownloadProgressEventArgs` (args)
- `svelters:winui/windows.applicationmodel.datatransfer`: `Clipboard` (statics), `ClipboardContentOptions` (object), `ClipboardHistoryItem` (object), `ClipboardHistoryItemsResult` (object), `DataPackage` (object), `DataPackagePropertySet` (model), `DataPackagePropertySetView` (object), `DataPackageView` (object), `OperationCompletedEventArgs` (args), `ShareCompletedEventArgs` (args), `ShareProvider` (object), `ShareTargetInfo` (object)
- `svelters:winui/windows.data.json`: `IJsonValue` (interface), `JsonArray` (model), `JsonObject` (model), `JsonValue` (object)
- `svelters:winui/windows.foundation`: `Deferral` (object), `IInspectable` (interface), `IWwwFormUrlDecoderEntry` (interface), `PropertyValue` (statics), `Uri` (object), `WwwFormUrlDecoder` (object)
- `svelters:winui/windows.foundation.collections`: `IMap` (collection), `IMapChangedEventArgs` (collection), `IObservableMap` (collection), `IVector` (collection), `PropertySet` (model), `StringMap` (model), `ValueSet` (model)
- `svelters:winui/windows.storage`: `FileIO` (statics), `IStorageFile` (interface), `IStorageFolder` (interface), `IStorageItem` (interface), `PathIO` (statics), `StorageFile` (object), `StorageFolder` (object), `StorageLibraryChange` (object), `StorageLibraryChangeReader` (object), `StorageLibraryChangeTracker` (object), `StorageLibraryChangeTrackerOptions` (object), `StorageProvider` (object), `StorageStreamTransaction` (object)
- `svelters:winui/windows.storage.fileproperties`: `BasicProperties` (object), `DocumentProperties` (object), `ImageProperties` (object), `MusicProperties` (object), `StorageItemContentProperties` (object), `StorageItemThumbnail` (object), `VideoProperties` (object)
- `svelters:winui/windows.storage.streams`: `IBuffer` (interface), `IInputStream` (interface), `IOutputStream` (interface), `IRandomAccessStream` (interface), `IRandomAccessStreamReference` (interface), `IRandomAccessStreamWithContentType` (interface), `RandomAccessStreamReference` (object)
- `svelters:winui/windows.ui`: `ColorHelper` (statics), `Colors` (statics)
