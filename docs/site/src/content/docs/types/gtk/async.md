---
title: Async calls and callbacks
description: "How GIO-style _async/_finish pairs become Promises, how cancellation and errors work, and how JS functions passed to GTK live and die."
---

GTK and GIO have two patterns that take a function: **async operations** (`g_file_load_contents_async` plus `g_file_load_contents_finish`) and **callbacks** (`gtk_custom_filter_set_filter_func`). The generated classes in `svelters:gtk/*` cover both. The backend reads the GIR's `scope`, `closure` and `destroy` annotations and pairs `_async` functions with their `_finish` functions, so no type has hand-written code for this.

## Async methods return Promises

```js
import { File } from 'svelters:gtk/gio';
import { FileDialog } from 'svelters:gtk/gtk';

const file = File.newForPath('/etc/hostname');
const { contents, etagOut } = await file.loadContentsAsync();
const info = await file.queryInfoAsync('standard::size', 'none');   // a FileInfo
const picked = await new FileDialog({ title: 'Open' }).open(window); // a File
```

- **One member per pair.** `foo_async` and `foo_finish` become one method named after the start function, such as `loadContentsAsync` or `open`. The finish function is not exposed.
- **The result** is what the finish function returns. If it also has out parameters, the result is an object with `ret` (when it returns a value) and one key per out parameter, the same shape as synchronous calls with out parameters.
- **Errors.** A `GError` from the finish function rejects the Promise with a `NativeError`. Its `domain` is the error quark (`g-io-error-quark`, `gtk-dialog-error-quark`) and its `code` is the member name of the domain's error enum (`not-found`, `cancelled`). Codes without an enum member stay numbers.
- **Priority.** A trailing `ioPriority` defaults to 0 (`G_PRIORITY_DEFAULT`).
- **Completion** runs on the GTK main loop, when GIO calls back. The backend calls the finish function and settles the Promise. Your `then`/`await` code runs at the next host checkpoint, like any other job. Nothing blocks.
- **Statics** work the same way (`File.newTmpAsync(null)` resolves with `{ ret, iostream }`).

## Cancelling

The native `GCancellable` parameter is hidden. Pass an `AbortSignal` in an options object as the last argument:

```js
const ac = new AbortController();
const pending = file.loadContentsAsync({ signal: ac.signal });
ac.abort();            // g_cancellable_cancel()
await pending;         // rejects: NativeError { domain: 'g-io-error-quark', code: 'cancelled' }
```

When there are optional parameters before it, pass them explicitly: `file.queryInfoAsync('standard::size', 'none', 0, { signal })`. A signal that is already aborted cancels right after the start. Each operation has its own `GCancellable`, so aborting one call leaves the others alone.

## Lifetime of an async call

- The object you call the method on stays alive while the call runs. A temporary is fine: `await File.newForPath(p).loadContentsAsync()`.
- If you `dispose()` the object, or the component that owns it is destroyed, while the call runs, the native operation is cancelled and the Promise rejects with `NativeError { domain: 'svelters', code: 'disposed' }`. An awaited call never hangs.
- Every call settles exactly once. Many concurrent calls on one object or many are independent.

## Callbacks

A callback parameter takes a plain JS function. Arguments that are objects arrive as generated classes. Use `Class.from(x)` to narrow a declared `GObject.Object`:

```js
import { CustomFilter, FilterListModel, StringList, StringObject } from 'svelters:gtk/gtk';

const list = new StringList({ strings: ['apple', 'banana'] });
const filter = new CustomFilter();
filter.setFilterFunc((item) => StringObject.from(item).string.startsWith('b'));
const model = new FilterListModel({ model: list, filter });
```

How long GTK may call the function depends on the GIR `scope` of the parameter:

| Scope | Example | GTK may call it | Released |
|---|---|---|---|
| `call` | `TextTagTable.foreach`, `File.measureDiskUsage` | during the call only | when the call returns |
| `async` | — | once, later | after that call |
| `notified` | `CustomFilter.setFilterFunc`, `MapListModel.new` | until GTK no longer needs it | when GTK calls the destroy notify |
| `forever` | (none in the bound set) | as long as the process runs | the native side never releases it; the JS function lives until the object it was passed to dies |

The JS function belongs to the object you passed it to. If that object is disposed while GTK still holds the callback, for example a filter that still sits inside a list model, later calls get a neutral answer (`false`, `0` or `null`), not a crash. The user data and destroy-notify parameters never appear in JS.

Tree operations (creating, inserting or destroying elements) are refused while GTK is inside a callback. Defer them with `queueMicrotask` or `setTimeout`.

## What is not bound

`metadata/gtk/types/report.txt` lists the members that stay blocked, with reasons. These include:

- async starts with out parameters, such as `InputStream.read_async` with its caller-allocated buffer;
- pairs whose finish function does not follow the `<name>_finish` naming, such as `File.replace_contents_bytes_async`;
- callbacks with untyped `gpointer` arguments, such as `ListStore.sort` and `CustomSorter`;
- callbacks that throw or take `GVariant` arguments.
