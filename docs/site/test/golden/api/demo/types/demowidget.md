---
title: "DemoWidget"
description: "Base class of all demo widgets."
sidebar:
  hidden: true
pagefind: true
editUrl: "https://example.invalid/repo/new/main/metadata/overrides/demo?filename=Widget.yaml"
---

<div class="svl-head"><span class="svl-toolkit svl-toolkit-demo">Demo Toolkit</span> <span class="svl-tier svl-tier-raw" title="Ingested from upstream metadata only">Raw ingested</span> <span class="svl-kind">class (abstract)</span></div>

:::caution[Raw ingested kind]
Raw: facts only.
You can promote it by adding an annotation: [create the override file](https://example.invalid/repo/new/main/metadata/overrides/demo?filename=Widget.yaml). See [the annotation workflow](/guides/annotation-workflow/).
:::

Base class of all demo widgets.

[Demo reference: Widget](https://demo.invalid/docs/widget.html)

## Dangerous members

:::danger[2 dangerous members]
These are reachable from app code (through `call()`, `setProp` or an attribute) unless marked otherwise, but the backend owns what they change: using them can corrupt the widget tree svelters manages, fight its lifecycle or crash. Nothing blocks them at run time yet.
:::

| Member | Kind | Severity | Why | Use instead |
| --- | --- | --- | --- | --- |
| [`visible`](#property-visible) | property (not reachable today) | owned by the backend | The backend toggles it on insert. | – |
| [`show`](#method-show) | method | corrupts the widget tree | Maps the widget behind the renderer's `show` attribute (&lt;demo&gt;). | The `visible` attribute or {#if}. |

## Properties

| JS name | Type | Default | Range | Access | Notify / bind | Origin | Version |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [`visible`](#property-visible) <span class="svl-danger" title="Dangerous: the backend owns what this changes">⚠ dangerous</span> | `boolean` | `true` | – | read / write | `notify::visible` · bindable | <span class="svl-badge svl-origin-gir" title="GObject-Introspection repository">gir</span> | – |

### Property details

<a id="property-visible"></a>
#### `visible`

<div class="svl-meta"><span class="svl-badge svl-origin-gir" title="GObject-Introspection repository">gir</span> </div>

Type `boolean` · read / write · default `true` · notify `notify::visible` (bindable: `bind:visible`, planned in F1)

:::danger[Dangerous: owned by the backend]
Writing this property is not possible from app code today; the backend owns this state, so do not expose it. The backend toggles it on insert.

<small>Declared in <code>metadata/overrides/demo/Widget.yaml</code>. Nothing blocks it at run time yet.</small>
:::

Whether the widget is shown. See [Align](/api/demo/types/demoalign/).


## Methods

| Name | Signature | Origin |
| --- | --- | --- |
| <a id="method-show"></a>`show` <span class="svl-danger" title="Dangerous: the backend owns what this changes">⚠ dangerous</span> | `show(): void` | <span class="svl-badge svl-origin-gir" title="GObject-Introspection repository">gir</span> |

#### `show`

`show(): void`

:::danger[Dangerous: corrupts the widget tree]
Calling `show` is reachable from app code (a native call or attribute), but the backend owns what it changes. Maps the widget behind the renderer's `show` attribute (&lt;demo&gt;).

**Use instead:** The `visible` attribute or {#if}.

<small>Declared in <code>metadata/dangerous/demo.yaml#show</code>. Nothing blocks it at run time yet.</small>
:::


## See also

- ABI: [`set_prop`](/concepts/abi/#set_prop)
- Serdes support: **unknown**

<div class="svl-notice">Demo docs, (c) the Demo authors. <a href="https://opensource.org/license/mit">MIT License</a>.</div>
