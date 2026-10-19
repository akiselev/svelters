---
title: "<button> (DemoButton)"
description: "A clickable button."
sidebar:
  hidden: true
pagefind: true
editUrl: "https://example.invalid/repo/edit/main/metadata/overrides/demo/Button.yaml"
---

<div class="svl-head"><span class="svl-toolkit svl-toolkit-demo">Demo Toolkit</span> <span class="svl-tier svl-tier-annotated" title="Hand/agent-annotated, with generated verification tests">Annotated &amp; tested</span> <code class="svl-tag">&lt;button&gt;</code> <span class="svl-kind">class</span> <a class="svl-kind" href="/concepts/ownership/#lifetime-classes">lifetime: owned</a></div>

:::tip[Annotated and tested]
Generated tests pass. [Edit this annotation](https://example.invalid/repo/edit/main/metadata/overrides/demo/Button.yaml).
:::

A clickable button.

[Demo reference: Button](https://demo.invalid/docs/button.html)

### Toolkit support

<table class="svl-matrix"><thead><tr><th>Demo Toolkit</th><th>egui</th></tr></thead><tbody><tr><td><a href="/api/demo/button/">✔ annotated &amp; tested</a></td><td><span class="svl-planned">planned</span></td></tr></tbody></table>

<figure class="svl-shot"><img src="/screenshots/demo/button.png" alt="Screenshot of &lt;button&gt; on Demo Toolkit" loading="lazy" width="320" /><figcaption>Demo Toolkit, rendered headless under Xvfb (generated).</figcaption></figure>

## Usage

**Using &lt;button&gt;** <span class="svl-kind" title="Generated from metadata; not compiled">generated, illustrative</span>

```svelte
<button label="Hi" onclick={() => {}} />
```

## Inheritance

**Chain:** Button → [Widget](/api/demo/types/demowidget/) → `Missing`

## Description

#### Heading

A button with a [label](/api/demo/button/#property-label) and an &lt;unknown&gt; tag.

```svelte
<button label="x" />
```

## Dangerous members

:::danger[2 dangerous members]
These are reachable from app code (through `call()`, `setProp` or an attribute) unless marked otherwise, but the backend owns what they change: using them can corrupt the widget tree svelters manages, fight its lifecycle or crash. Nothing blocks them at run time yet.
:::

| Member | Kind | Severity | Why | Use instead |
| --- | --- | --- | --- | --- |
| [`visible`](/api/demo/types/demowidget/#property-visible) <small>from Widget</small> | property (not reachable today) | owned by the backend | The backend toggles it on insert. | – |
| [`show`](/api/demo/types/demowidget/#method-show) <small>from Widget</small> | method | corrupts the widget tree | Maps the widget behind the renderer's `show` attribute (&lt;demo&gt;). | The `visible` attribute or {#if}. |

## Properties

| JS name | Type | Default | Range | Access | Notify / bind | Origin | Version |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [`label`](#property-label) ★ | `string` or `null` | `""` | – | read / write | `notify::label` · bindable | <span class="svl-badge svl-origin-gir" title="GObject-Introspection repository">gir</span> <span class="svl-badge svl-origin-manual" title="Hand/agent annotation (metadata/overrides)">manual</span> | since 1.1 |

### Property details

<a id="property-label"></a>
#### `label`

<div class="svl-meta"><span class="svl-badge svl-origin-gir" title="GObject-Introspection repository">gir</span> <span class="svl-badge svl-origin-manual" title="Hand/agent annotation (metadata/overrides)">manual</span> <span class="svl-kind">annotated</span> </div>

Type `string` or `null` · read / write · default `""` · notify `notify::label` (bindable: `bind:label`, planned in F1) · since 1.1


### Inherited properties

<details><summary>From Widget (1)</summary>

| JS name | Type | Access | Description |
| --- | --- | --- | --- |
| [`visible`](/api/demo/types/demowidget/#property-visible) <span class="svl-danger" title="Dangerous: the backend owns what this changes">⚠ dangerous</span> | `boolean` | read / write | Whether the widget is shown. See Align. |

</details>


## Events

| Event | Handler attribute | Arguments | Returns | Sync / result | Phase | Origin |
| --- | --- | --- | --- | --- | --- | --- |
| [`clicked`](#event-clicked) ★ | `onclicked` / `onclick` | – | void | async | `first` | <span class="svl-badge svl-origin-gir" title="GObject-Introspection repository">gir</span> |

### Event details

<a id="event-clicked"></a>
#### `clicked`

<div class="svl-meta"><span class="svl-badge svl-origin-gir" title="GObject-Introspection repository">gir</span> <span class="svl-kind">annotated</span></div>

Handler: `onclicked` / `onclick` · arguments: – · returns `void` · async · phase `first`

Emitted when clicked.


## Children and slots

**Policy:** `single_child`, default slot `child`

| Slot | Cardinality | Accepts | Operations | Ownership effects |
| --- | --- | --- | --- | --- |
| `child` (default) <span class="svl-badge svl-origin-manual" title="Hand/agent annotation (metadata/overrides)">manual</span> | one | Widget | **set**: `set_child(Widget)` | **set**: child parented |

## Lifetime

Class **owned** <span class="svl-badge svl-origin-derived" title="Derived by the docs generator">derived</span>: Owned by its parent. See [Ownership and lifetimes](/concepts/ownership/).

## Demo specifics

**CSS node:** `button`

### Notes

Styled through the `button` node.

## See also

- ABI: [`create`](/concepts/abi/#create), [`destroy`](/concepts/abi/#destroy), [`listen`](/concepts/abi/#listen), [`set_prop`](/concepts/abi/#set_prop)
- Serdes support: **unknown**

<div class="svl-notice">Demo docs, (c) the Demo authors. <a href="https://opensource.org/license/mit">MIT License</a>.</div>
