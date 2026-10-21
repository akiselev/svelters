---
title: Non-UI types (GTK)
description: "Gio, GLib, Gdk, Gsk, Pango and Gtk classes as ES module named exports (svelters:gtk/gio, ...)."
---

Every non-UI GTK type is a named export of a `svelters:gtk/<namespace>` module. Elements stay tags; objects, models and values are imported:

```svelte
<script>
  import { TextBuffer } from 'svelters:gtk/gtk';
  const buffer = new TextBuffer({ text: 'Hello' });
</script>
<text-view {buffer} />
```

- **Objects** (`Gio.File`, `Gtk.StringList`) are SHARED handles: one JS object per native object, `===` works, the object lives while the wrapper or anything else holds it.
- **Values** (`Pango.FontDescription`, `Gdk.ContentFormats`) are VALUE handles: immutable, no identity, `equals`, `toString`, `dispose`.
- **Release builds** keep only what an app imports, plus the types its signatures mention. The manifest is `packages/types/gtk/manifest.json`.

- **Async methods** (`file.loadContentsAsync()`, `dialog.open(window)`) return Promises; `{ signal }` cancels. **Callbacks** are plain JS functions. See [Async calls and callbacks](async/).

The curated set below has a page and ledger-verified tests; the other bound types are generated from the same facts.

## Curated types

- [Gdk.Clipboard](gdk-clipboard/)
- [Gdk.Texture](gdk-texture/)
- [Gio.File](gio-file/)
- [Gio.ListStore](gio-liststore/)
- [Gtk.CustomFilter](gtk-customfilter/)
- [Gtk.CustomSorter](gtk-customsorter/)
- [Gtk.DirectoryList](gtk-directorylist/)
- [Gtk.FileDialog](gtk-filedialog/)
- [Gtk.FilterListModel](gtk-filterlistmodel/)
- [Gtk.MultiSelection](gtk-multiselection/)
- [Gtk.SingleSelection](gtk-singleselection/)
- [Gtk.SortListModel](gtk-sortlistmodel/)
- [Gtk.StringList](gtk-stringlist/)
- [Gtk.TextBuffer](gtk-textbuffer/)
- [Pango.AttrList](pango-attrlist/)
- [Pango.FontDescription](pango-fontdescription/)

## Bound types by namespace

- `svelters:gtk/glib` (14): Bytes, DateTime, Error, HashTable, KeyFile, OptionGroup, PollFD, String, TimeZone, Tree, Uri, VariantBuilder, VariantDict, VariantType
- `svelters:gtk/gobject` (3): InitiallyUnowned, Object, Value
- `svelters:gtk/gdk` (25): AppLaunchContext, CairoContext, CicpParams, Clipboard, ColorState, ContentFormats, ContentProvider, Cursor, Device, DeviceTool, Display, DmabufFormats, Drag, DrawContext, Drop, EventSequence, FrameClock, FrameTimings, GLContext, Monitor, Paintable, Seat, Snapshot, Surface, Texture
- `svelters:gtk/gio` (57): Action, ActionGroup, ActionMap, AppInfo, AppLaunchContext, Application, ApplicationCommandLine, AsyncInitable, Cancellable, Credentials, DBusAnnotationInfo, DBusArgInfo, DBusAuthObserver, DBusConnection, DBusInterfaceInfo, DBusMessage, DBusMethodInfo, DBusPropertyInfo, DBusSignalInfo, Drive, File, FileAttributeInfoList, FileAttributeMatcher, FileEnumerator, FileIOStream, FileIcon, FileInfo, FileInputStream, FileMonitor, FileOutputStream, IOStream, Icon, Initable, InputStream, ListModel, ListStore, LoadableIcon, Menu, MenuAttributeIter, MenuItem, MenuLinkIter, MenuModel, Mount, MountOperation, Notification, OutputStream, Permission, Seekable, Settings, SettingsBackend, SettingsSchema, SettingsSchemaKey, SimpleAction, SimpleActionGroup, ThemedIcon, UnixFDList, Volume
- `svelters:gtk/graphene` (13): Box, Euler, Matrix, Plane, Point3D, Quad, Quaternion, Ray, Sphere, Triangle, Vec2, Vec3, Vec4
- `svelters:gtk/gsk` (7): ComponentTransfer, Path, PathMeasure, PathPoint, Renderer, Stroke, Transform
- `svelters:gtk/gtk` (166): ATContext, Accessible, AccessibleHyperlink, AccessibleHypertext, AccessibleList, AccessibleRange, AccessibleText, Actionable, ActivateAction, Adjustment, AlertDialog, AlternativeTrigger, AnyFilter, Application, BinLayout, Bitset, BookmarkList, BoolFilter, BoxLayout, Buildable, Builder, BuilderCScope, BuilderListItemFactory, BuilderScope, CallbackAction, CenterLayout, ColorDialog, ColumnViewCell, ColumnViewColumn, ColumnViewRow, ColumnViewSorter, Constraint, ConstraintGuide, ConstraintLayout, ConstraintLayoutChild, ConstraintTarget, CssProvider, CssSection, CustomFilter, CustomLayout, CustomSorter, DirectoryList, DragSource, DropControllerMotion, DropTarget, DropTargetAsync, Editable, EntryBuffer, EnumList, EnumListItem, EventController, EventControllerFocus, EventControllerKey, EventControllerLegacy, EventControllerMotion, EventControllerScroll, EveryFilter, ExpressionWatch, FileDialog, FileFilter, FileLauncher, Filter, FilterListModel, FixedLayout, FixedLayoutChild, FlattenListModel, FontDialog, Gesture, GestureClick, GestureDrag, GestureLongPress, GesturePan, GestureRotate, GestureSingle, GestureStylus, GestureSwipe, GestureZoom, GridLayout, GridLayoutChild, IMContext, IMContextSimple, IMMulticontext, IconPaintable, IconTheme, KeyvalTrigger, LayoutChild, LayoutManager, ListHeader, ListItem, ListItemFactory, MapListModel, MediaFile, MediaStream, MnemonicAction, MnemonicTrigger, MountOperation, MultiFilter, MultiSelection, MultiSorter, NamedAction, Native, NativeDialog, NeverTrigger, NoSelection, NotebookPage, NothingAction, NumericSorter, Orientable, OverlayLayout, OverlayLayoutChild, PadController, PageSetup, PaperSize, PasswordEntryBuffer, PrintContext, PrintDialog, PrintJob, PrintOperation, PrintOperationPreview, PrintSettings, PrintSetup, Printer, RecentInfo, RecentManager, Requisition, Root, ScrollInfo, Scrollable, SectionModel, SelectionFilterModel, SelectionModel, Settings, Shortcut, ShortcutAction, ShortcutController, ShortcutManager, ShortcutTrigger, SignalAction, SignalListItemFactory, SingleSelection, SizeGroup, SliceListModel, Snapshot, SortListModel, Sorter, StackPage, StringFilter, StringList, StringObject, StringSorter, StyleProvider, Svg, SymbolicPaintable, TextBuffer, TextChildAnchor, TextIter, TextMark, TextTag, TextTagTable, Tooltip, TreeListModel, TreeListRow, TreeListRowSorter, UriLauncher, WidgetPaintable, WindowGroup
- `svelters:gtk/pango` (17): AttrIterator, AttrList, Attribute, Context, Coverage, Font, FontDescription, FontFace, FontFamily, FontMap, FontMetrics, Fontset, Language, Layout, LayoutLine, Matrix, TabArray
