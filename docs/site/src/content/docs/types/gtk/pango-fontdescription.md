---
title: Pango.FontDescription
description: "Pango.FontDescription from svelters:gtk/pango, a value class generated from the GIR."
---

```js
import { FontDescription } from 'svelters:gtk/pango';
```

## Class

`Pango.FontDescription`, GType `PangoFontDescription`.

Lifetime: a **VALUE** handle: an immutable snapshot with no identity. Its mutating methods are not exposed. `dispose()` releases it now, otherwise the garbage collector does.

Facts come from the GIR; the tests behind them are listed in the ledger (`metadata/verified/gtk.json`, fact ids `svelters-api:v1:type:gtk:Pango:FontDescription:*`).

## Statics

- `FontDescription.new()`
- `FontDescription.fromString(str)`

## Methods

- `betterMatch(oldMatch, newMatch)`
- `copyStatic()`
- `equal(desc2)`
- `getColor()`, since 1.57
- `getFamily()`
- `getFeatures()`, since 1.56
- `getGravity()`, since 1.16
- `getSetFields()`
- `getSize()`
- `getSizeIsAbsolute()`, since 1.8
- `getStretch()`
- `getStyle()`
- `getVariant()`
- `getVariations()`, since 1.42
- `getWeight()`
- `getWidth()`, since 1.58
- `hash()`
- `toFilename()`
- `toString()`

## Hidden mutators

Value handles are immutable in JS; these native methods are not exposed: `merge`, `merge_static`, `set_absolute_size`, `set_color`, `set_family`, `set_family_static`, `set_features`, `set_features_static`, `set_gravity`, `set_size`, `set_stretch`, `set_style`, `set_variant`, `set_variations`, `set_variations_static`, `set_weight`, `set_width`, `unset_fields`.

