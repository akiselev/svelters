---
title: QUrl
description: "QUrl from svelters:qt/core, a value class generated from qdoc, PySide6 and moc facts."
---

```js
import { QUrl } from 'svelters:qt/core';
```

## Class

`QUrl` (QtCore, `qurl.h`), category **value**.

Lifetime: an immutable **VALUE** handle: a copy of the Qt value, no identity (`equals()` compares natively), a new handle per crossing, released by `dispose()` or garbage collection. Non-const members are hidden (decision Q3).

Facts: `metadata/qt/types/QtCore.json`; ledger fact ids `svelters-api:v1:type:qt:QtCore:QUrl:*` in `metadata/verified/qt.json`.

## Construct

Constructors (one JS constructor; the backend picks the overload from the arguments):

- `new QUrl()`
- `new QUrl(url, parsingMode)`

## Statics

- `QUrl.fromAce(domain, options)` (since Qt 6.3)
- `QUrl.fromEncoded(input, mode)`
- `QUrl.fromLocalFile(localFile)`
- `QUrl.fromPercentEncoding(input)`
- `QUrl.fromStringList(urls, mode)`
- `QUrl.fromUserInput(userInput, workingDirectory, options)`
- `QUrl.idnWhitelist()`
- `QUrl.setIdnWhitelist(list)`
- `QUrl.toAce(domain, options)` (since Qt 6.3)
- `QUrl.toPercentEncoding(input, exclude, include)`
- `QUrl.toStringList(urls)`

## Methods

- `authority()`
- `errorString()`
- `fileName()`
- `fragment()`
- `hasFragment()`
- `hasQuery()`
- `host()`
- `isEmpty()`
- `isLocalFile()`
- `isParentOf(childUrl)`
- `isRelative()`
- `isValid()`
- `password()`
- `path()`
- `port(defaultPort)`
- `query()`
- `resolved(relative)`
- `scheme()`
- `toDisplayString()`
- `toEncoded()`
- `toLocalFile()`
- `toString()`
- `url()`
- `userInfo()`
- `userName()`

## Hidden

Reached otherwise (a property) or mutators of a value class: `clear`, `setAuthority`, `setFragment`, `setHost`, `setPassword`, `setPath`, `setPort`, `setQuery`, `setScheme`, `setUrl`, `setUserInfo`, `setUserName`.

## Not bound

- `QUrl.adjusted(QUrl::FormattingOptions)`: blocked-type:QUrl::FormattingOptions
- `QUrl.matches(const QUrl &, QUrl::FormattingOptions)`: blocked-type:QUrl::FormattingOptions
