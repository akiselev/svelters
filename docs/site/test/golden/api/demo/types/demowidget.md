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

## Properties

| JS name | Type | Default | Range | Access | Notify / bind | Origin | Version |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [`visible`](#property-visible) | `boolean` | `true` | – | read / write | `notify::visible` · bindable | <span class="svl-badge svl-origin-gir" title="GObject-Introspection repository">gir</span> | – |

### Property details

<a id="property-visible"></a>
#### `visible`

<div class="svl-meta"><span class="svl-badge svl-origin-gir" title="GObject-Introspection repository">gir</span> </div>

Type `boolean` · read / write · default `true` · notify `notify::visible` (bindable: `bind:visible`, planned in F1)

Whether the widget is shown. See [Align](/api/demo/types/demoalign/).


## Methods

| Name | Signature | Origin |
| --- | --- | --- |
| <a id="method-show"></a>`show` | `show(): void` | <span class="svl-badge svl-origin-gir" title="GObject-Introspection repository">gir</span> |

## See also

- ABI: [`set_prop`](/concepts/abi/#set_prop)
- Serdes support: **unknown**

<div class="svl-notice">Demo docs, (c) the Demo authors. <a href="https://opensource.org/license/mit">MIT License</a>.</div>
