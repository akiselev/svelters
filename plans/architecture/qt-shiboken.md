# QtWidgets extraction through Shiboken (Q2)

QtWidgets cannot be covered by QMetaObject reflection alone. Many useful constructors and ordinary C++ methods are neither properties, signals, slots, nor `Q_INVOKABLE`. Svelters therefore needs selective C++ thunk generation, but it should not rediscover Qt's binding semantics from raw headers.

## Build-time architecture

```text
Qt headers + exact Qt build configuration
             |
PySide/Shiboken typesystem knowledge
             |
             v
Shiboken ApiExtractor
             |
             v
svelters-qt-extract exporter
             |
             v
canonical Svelters API JSON
             |
        +----+-----+
        |          |
       .d.ts    binding linker
                   |
                   v
             thin C++ thunks
```

Shiboken stays on the build/tool side. The shipped application does not link Shiboken or Python.

## Why ApiExtractor

ApiExtractor already models classes, overloads, enums, global functions, value types, implicit conversions, and the modifications made by the Shiboken type database. PySide's QtWidgets typesystem adds years of hand-maintained knowledge about removed overloads, parent/ownership relationships, changed defaults, conversions, polymorphism, and custom glue.

Raw libclang extraction alone would lose those semantics.

## Export strategy

Build `tools/qt-extract` as a small C++ executable using the supported Shiboken generator/ApiExtractor interfaces for the pinned Qt/PySide version. It serializes only neutral facts into the canonical API IR.

Do not fork the Python generator. Do not generate CPython wrappers and parse them back.

The extractor records:

- Qt version, module, include configuration and ABI target;
- Shiboken/PySide version/commit;
- every typesystem file hash loaded;
- classes/functions/enums and exact signatures;
- ownership/parent-management modifications;
- availability/since/removed metadata;
- conversions/default arguments;
- generated/custom snippets only as provenance until Svelters explicitly maps them.

Custom PySide glue is not automatically copied into Svelters. Each required semantic behavior gets a Svelters-native adapter or converter with tests.

## Licensing boundary

The Shiboken/PySide source tree contains mixed licensing, including files under Qt's GPL exception and PySide typesystem files under Qt commercial/LGPL/GPL terms. Treat the extractor as a separately accounted build tool, preserve all required notices, and keep a provenance manifest for every generated API database.

Before distributing pre-generated API databases or extractor binaries, complete a license review for the exact Qt/PySide versions and intended distribution model. Do not assume that because one ApiExtractor file is BSD the whole extraction stack or its copied snippets are BSD.

No PySide injected source snippet enters Svelters generated runtime code by default.

## Qt invocation classes

Every callable is classified:

```text
Q_PROPERTY read/write     -> generic metaobject/property path
signal                    -> generic/generated signal connector
slot / Q_INVOKABLE        -> metaobject invocation where sound
ordinary C++ method       -> generated direct C++ thunk
constructor               -> generated direct C++ thunk
special conversion/life   -> Svelters converter/custom adapter
unsupported               -> explicit reason
```

QMetaObject is an optimization/capability path, not the definition of the API surface.

## Thin C++ ABI

Generated code talks to Svelters through stable handles and explicit C-compatible values. No Rust trait object or `QObject*` enters JS.

Example shape:

```cpp
extern "C" SvStatus sv_binding_17(
    SvBackend* backend,
    SvHandle receiver,
    const SvValue* args,
    size_t nargs,
    SvValue* out);
```

The thunk resolves `receiver` to the expected QObject subclass, validates arguments, performs typed conversions, invokes the exact C++ overload, records/updates ownership if needed, and converts the result.

## Fat development bridge

Generate a broad QtWidgets bridge once per API-database/toolchain fingerprint and cache it with Artifactum. It may be substantially larger than release output; its purpose is to let a developer introduce another Qt call without compiling C++ during normal `.svelte` iteration.

The release binding linker replaces it with a thin bridge rooted at surviving JS imports.

## Coverage accounting

Generate a machine-readable coverage report:

```text
metaobject-reflected
fat-bridge generated
thin-thunk supported
custom adapter
unsupported
unknown/unclassified
```

Do not claim QtWidgets support based on widget count alone. Track callable coverage and the unsupported reason distribution.
