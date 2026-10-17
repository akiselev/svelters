---
title: "API reference"
description: "Generated element and type reference for every toolkit"
sidebar:
  label: Overview
---

The API reference is generated from the widget metadata (`metadata/`), the documentation-text pass (`metadata/docs/`) and the annotations (`metadata/overrides/`). Every page shows where each fact came from.

| Toolkit | Version | Elements | Annotated & tested | Kinds ingested | Properties | Events | Methods |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [Demo Toolkit](/api/demo/) | 1.2.3 | 1 | 1 | 3 | 2 | 1 | 1 |

Also: the [element support matrix](/api/matrix/) shows which elements exist in which toolkit, and the [coverage dashboard](/coverage/) tracks annotation progress.

:::caution[What "annotated and tested" means]
Only the prototype kinds (`window`, `box`, `label`, `button`, `entry`, `scrolled-window` and a few more) are hand-annotated with generated verification tests. All other kinds are **raw**: facts straight from GIR, qdoc and the QMetaObject. See [the annotation workflow](/guides/annotation-workflow/).
:::
