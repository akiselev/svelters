# Inline direction (`dir`, `direction`, `:dir()`)

skelter lays out right-to-left the way the web does (POL4-DIR-CORE). One value, the box's inline direction, decides
where a line starts, which end of a `row` comes first, where `start` and `end` are, and how a kind orders its parts.

## The model

- **CSS `direction: ltr | rtl`** is an inherited property; its initial value is `ltr`.
- **`dir` on any element** (`ltr`, `rtl` or `auto`) sets it, below author rules: `<box dir="rtl">` and every
  descendant without a `dir` of its own lay out right to left, and a `direction` declaration in a rule still wins on
  that element. `auto` takes the direction of the first strong character (a letter of a left-to-right or a
  right-to-left script; digits and punctuation do not count) of the element's text, then of its descendants' text in
  tree order, skipping descendants that have a `dir` of their own; with no strong character it is `ltr`. Any other
  value is no `dir`.
- **A window's root** takes the document's direction, a host setting: `Document::set_direction(Direction::Rtl)`
  (default `ltr`). Changing it restyles every window.
- **`:dir(ltr)` / `:dir(rtl)`** match an element's *directionality*, HTML's: its `dir`, else its nearest ancestor's,
  else the document's. As in browsers, a `direction` declaration does not change what `:dir()` matches, so write
  `:dir(rtl)` rules against `dir`, not against `direction`. `[dir=rtl]` matches the attribute itself.

```svelte
<box dir="rtl" class="toolbar">
  <button>حفظ</button>
  <button>إلغاء</button>
</box>

<style>
  .toolbar { flex-direction: row; gap: 4px; padding-inline-start: 8px }
  .icon:dir(rtl) { transform: scaleX(-1) }
</style>
```

## What follows the direction

| What | In `rtl` |
|---|---|
| `flex-direction: row` | lays out from the right (`row` runs from the inline start to the end); `row-reverse` from the left |
| `justify-content: start / end`, `flex-start / flex-end` | on a row's main axis: `start` is the right |
| `align-items` / `align-self: start / end` | on a column's cross axis (the inline axis): `start` is the right |
| grid columns | run from the right (a grid kind's tracks) |
| `text-align: start / end` | `start` (the default) is the right; `left`, `right`, `center` stay physical |
| text | an `rtl` box's paragraphs get a right-to-left base: mixed runs order from the right (`abc שלום` shows `abc` at the right end) |
| logical properties | `margin-inline-start/end`, `padding-inline-start/end`, `inset-inline-start/end`, `border-inline-start/end-{width,style,color}` and the shorthands `margin-inline`, `padding-inline`, `inset-inline`, `border-inline`, `border-inline-start`, `border-inline-end`: `start` is the right |
| a horizontal scroll container | starts at its inline start (the right end of its content); content overflows to the left; `:state(sv.overflow-left)` says there is more to the left |
| AccessKit | an `rtl` node carries `text_direction: RightToLeft` (an `ltr` node inside an `rtl` one, `LeftToRight`) |

Physical properties (`margin-left`, `padding-right`, `left`, `right`, `border-left-*`) stay physical. When an element
sets both a logical side and the physical side it resolves to, the logical one wins (CSS takes the later declaration;
skelter's computed style no longer knows the order).

Not supported yet: the `ms-`/`me-`/`ps-`/`pe-` utility classes (the utility table lists them as unsupported), vertical
writing modes, `unicode-bidi`, and a base direction for `ltr` boxes: an `ltr` box's text still takes its paragraph
direction from its first strong character, so Arabic text in an `ltr` document reads from the right, as it did before
direction existed.

## Scroll offsets

A scroll container's offset stays left-based in every API (`Document::scroll_offset`, `ScrollGeometry::offset`, the
compositor's scroll nodes): 0 is the content's left edge. In an `rtl` container the offset starts at its range (the
right end) and keeps its distance from the right end while the content grows or shrinks; the content space is shifted
by the range, because layout places the inline start at the right and lets the overflow run to the left. A kind that
draws scroll bars from `ScrollGeometry` needs no change for the thumb; Chrome puts the vertical bar at the left in
`rtl`, which the kind decides (`NodeView::direction()`).

## How a kind mirrors

A kind reads `NodeView::direction()` (or `is_rtl()`), never its text, and mirrors what it places or paints itself:

- **part order**: a checkbox's box goes on the right of its caption, a tree's twisty on the right of its row, a menu
  item's shortcut on the left;
- **chevrons and arrows**: "forward" points left (a submenu chevron, a breadcrumb separator, a collapsed twisty);
- **fills**: a progress bar or slider fills from the right;
- **keys**: Left and Right swap where they mean "previous" and "next" (a slider, a tab strip), not where they mean
  "move the caret visually";
- **text**: pass the direction into the rich text a kind lays out (`skelter_text::RichText::direction`), so `start`
  alignment and run order follow the box.

A kind's children already lay out mirrored (they are flex or grid children): only what the kind draws or places by
hand needs code. A kind that builds its own taffy style in `layout_style` keeps the node's direction (core sets it
after the kind).

## Cost

Direction is computed in the style pass, for the nodes a restyle already visits: one `dir` lookup per node, and for an
element with a `dir` that changes the direction (or an `rtl` window root), one copy of the parent's computed style.
Nothing runs per frame. A `dir` change restyles that element's subtree; a text change looks for the nearest `dir` above
it only once some node has had `dir=auto`.
