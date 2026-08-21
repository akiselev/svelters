# SVR11 - Qt/Shiboken API extraction

References: `plans/architecture/qt-shiboken.md`, `plans/architecture/native-api-ir.md`.

## Goal

Implement Q2's Qt side: extract QtWidgets API semantics through Shiboken/PySide knowledge into the neutral Svelters API IR without making PySide/Shiboken a runtime dependency.

## Work

Build a standalone `svelters-qt-extract` tool around the Shiboken generator/ApiExtractor interfaces for a pinned Qt 6/PySide version.

Start with QtCore, QtGui and QtWidgets. Export exact classes, bases, methods/constructors, overloads, enums/flags, signals/properties, implicit conversions, defaults, const/reference qualifiers, availability, and ownership/parent-management modifications.

Build a translator for PySide typesystem semantic modifications. Do not copy injected Python/C++ glue blindly. Record injected/custom snippets as provenance and mark the affected API entries `CustomAdapterRequired` until a Svelters-native implementation exists.

Create the license/provenance report required by `qt-shiboken.md` and a distribution decision for extractor binaries/API DB artifacts before publishing them outside CI/dev use.

Generate `@svelters/qtwidgets` `.d.ts` from the canonical IR.

## Validation

Compare extracted signatures against a representative set of Qt documentation/classes including QWidget, layouts, QMainWindow, QAction/menu types, QTreeView, QListView, QTableView, QFileDialog and model/view base classes.

## Acceptance

- deterministic API database for a pinned Qt build;
- ownership/parent changes from typesystems are visible in IR tests;
- ordinary non-Q_INVOKABLE methods appear as callable records needing generated thunks;
- coverage/unknown-modification report is zero or explicitly triaged for the reference class set;
- generated typings type-check native Qt Svelte examples.
