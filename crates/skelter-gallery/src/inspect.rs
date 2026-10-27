//! `inspect`: one node of a mounted cell as JSON: its kind, anchor, layout box, computed style
//! (every property that differs from CSS's initial value), the rules that match it, the state bits
//! forced on it, and its AccessKit node.

use crate::model::{Cell, State};
use crate::render::{Error, Renderer};
use serde_json::{Value, json};
use skelter_base::color::{Color, Fill};
use skelter_base::ids::NodeId;
use skelter_core::ComputedBox;
use skelter_base::ids::WindowId;
use skelter_test::A11yTree;
use svelters_abi::svl_handle;
use svelters_skelter::SkelterBackend;
use svelters_style::{MatchedRuleInfo, StateMask, StyleSource};

fn hex(c: Color) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    if c.a >= 1.0 { format!("#{:02x}{:02x}{:02x}", b(c.r), b(c.g), b(c.b)) } else { format!("#{:02x}{:02x}{:02x}{:02x}", b(c.r), b(c.g), b(c.b), b(c.a)) }
}

fn fill(f: &Fill) -> String {
    match f {
        Fill::Solid(c) => hex(*c),
        other => format!("{other:?}"),
    }
}

/// The properties of `c` that differ from CSS's initial values (`ComputedBox::default`), by CSS-ish
/// name, as text.
pub fn computed_diff(c: &ComputedBox) -> serde_json::Map<String, Value> {
    let d = ComputedBox::default();
    let mut out = serde_json::Map::new();
    let mut put = |name: &str, differs: bool, v: String| {
        if differs {
            out.insert(name.into(), Value::String(v));
        }
    };
    put("color", c.color != d.color, hex(c.color));
    put("caret-color", c.caret_color != d.caret_color, c.caret_color.map_or("auto".into(), hex));
    put("font-family", c.font_family != d.font_family, c.font_family.to_string());
    put("font-size", c.font_size != d.font_size, format!("{}px", c.font_size));
    put("font-weight", c.font_weight != d.font_weight, c.font_weight.to_string());
    put("font-style", c.italic != d.italic, if c.italic { "italic" } else { "normal" }.into());
    put("line-height", c.line_height != d.line_height, c.line_height.map_or("normal".into(), |l| l.to_string()));
    put("letter-spacing", c.letter_spacing != d.letter_spacing, format!("{}px", c.letter_spacing));
    put("white-space", c.wrap != d.wrap, if c.wrap { "normal" } else { "nowrap" }.into());
    put("text-align", c.text_align != d.text_align, format!("{:?}", c.text_align).to_lowercase());
    put("cursor", c.cursor != d.cursor, format!("{:?}", c.cursor));
    put("visibility", c.visibility_hidden != d.visibility_hidden, if c.visibility_hidden { "hidden" } else { "visible" }.into());
    put("pointer-events", c.pointer_events_none != d.pointer_events_none, if c.pointer_events_none { "none" } else { "auto" }.into());
    put("background", c.background != d.background, fill(&c.background));
    let bw = c.border_widths;
    put("border-width", bw != d.border_widths, format!("{}px {}px {}px {}px", bw.top, bw.right, bw.bottom, bw.left));
    put("border-color", c.border_color != d.border_color, hex(c.border_color));
    put("border-style", c.border_style != d.border_style, format!("{:?}", c.border_style).to_lowercase());
    put("border-radius", c.radii != d.radii, format!("{:?}", c.radii));
    put("box-shadow", c.shadows != d.shadows, format!("{:?}", c.shadows));
    put("opacity", c.opacity != d.opacity, c.opacity.to_string());
    put("transform", c.transform != d.transform, format!("{:?}", c.transform));
    put("transform-origin", c.transform_origin != d.transform_origin, format!("{:?}", c.transform_origin));
    put("overflow-x", c.overflow_x != d.overflow_x, format!("{:?}", c.overflow_x).to_lowercase());
    put("overflow-y", c.overflow_y != d.overflow_y, format!("{:?}", c.overflow_y).to_lowercase());
    put("z-index", c.z_index != d.z_index, c.z_index.map_or("auto".into(), |z| z.to_string()));
    put("will-change", c.will_change != d.will_change, "transform".into());
    put("outline-width", c.outline_width != d.outline_width, format!("{}px", c.outline_width));
    put("outline-color", c.outline_color != d.outline_color, hex(c.outline_color));
    put("outline-offset", c.outline_offset != d.outline_offset, format!("{}px", c.outline_offset));
    put("transition", c.transitions != d.transitions, format!("{:?}", c.transitions));
    out
}

/// The pseudo-classes whose bits are set in `bits`.
pub fn state_names(bits: StateMask) -> Vec<&'static str> {
    State::ALL
        .into_iter()
        .filter_map(|s| s.pseudo_class())
        .filter(|p| crate::render::pseudo_bits(p).is_some_and(|b| !b.is_empty() && bits.contains(b)))
        .collect()
}

/// `src` as `{file, line}`: the line of the `<style>` offset in the component file as it is now.
fn source_json(src: &StyleSource) -> Value {
    let line = std::fs::read_to_string(&src.file).ok().and_then(|text| {
        let open = text.find("<style")?;
        let content = open + text[open..].find('>')? + 1;
        let at = (content + src.style_offset as usize).min(text.len());
        Some(text[..at].matches('\n').count() + 1)
    });
    json!({ "file": src.file, "line": line, "style_offset": src.style_offset })
}

fn rule_json(r: &MatchedRuleInfo) -> Value {
    json!({
        "selector": r.selectors,
        "sheet": r.sheet,
        "layer": r.layer,
        "specificity": r.specificity,
        "sheet_line": r.line,
        "source": r.source.as_ref().map(source_json),
        "declarations": r.decls.iter().map(|d| json!({
            "name": d.name,
            "value": d.value,
            "important": d.important,
            "winner": d.winner,
            "source": d.source.as_ref().map(source_json),
        })).collect::<Vec<_>>(),
    })
}

/// The handles of the nodes under `frame` (a cell's slot or root, included) that `selector` picks:
/// `#anchor` or a kind name (the JS tag, or skelter-core's name), in tree order. For a host that
/// mounts cells into its own document (the gallery app).
pub fn select_in(backend: &SkelterBackend, frame: NodeId, selector: &str) -> Vec<svl_handle> {
    let sel = selector.trim();
    let mut order = Vec::new();
    backend.with_doc(|d| {
        let mut stack = vec![frame];
        while let Some(n) = stack.pop() {
            order.push(n);
            stack.extend(d.children(n).iter().rev().copied());
        }
    });
    backend.scene().read(|s| {
        order
            .into_iter()
            .filter_map(|n| s.get_node(n))
            .filter(|n| match sel.strip_prefix('#') {
                Some(a) => n.anchor().as_deref() == Some(a),
                None => n.tag() == sel || svelters_skelter::kinds::core_tag(n.tag()) == sel,
            })
            .map(|n| n.id())
            .collect()
    })
}

/// One node as `inspect` prints it (an element of its `nodes`): kind, anchor, boxes (`window`'s last
/// frame), computed style, matched rules, state, and the AccessKit node from `a11y` when given.
/// `None` for a stale handle.
pub fn node_json(backend: &SkelterBackend, window: Option<WindowId>, h: svl_handle, a11y: Option<&A11yTree>) -> Option<Value> {
    let (n, tag, anchor, drawn, rules, forced) = backend.scene().read(|s| {
        let r = s.get(h)?;
        Some((r.node(), r.tag(), r.anchor(), r.drawn(), r.matched_rules(), r.forced_state()))
    })?;
    let shown = window.and_then(|w| backend.scene().read(|s| s.shown(w, h)));
    let (core_kind, layout, computed, own) = backend.with_doc(|d| {
        let kind = d.kind_of(n).and_then(|k| d.registry().get(k)).map(|e| e.spec.name);
        (kind, d.layout_rect(n), d.computed(n).map(computed_diff), d.state(n))
    });
    let rect = |r: skelter_base::geom::Rect| json!({ "x": r.min.x, "y": r.min.y, "w": r.max.x - r.min.x, "h": r.max.y - r.min.y });
    let ak = a11y.and_then(|t| {
        let info = t.walk().into_iter().find(|i| i.id.0 == n.0)?;
        Some(json!({
            "role": format!("{:?}", info.role()),
            "name": info.label(),
            "value": info.value(),
            "author_id": info.author_id(),
            "disabled": info.is_disabled(),
            "hidden": info.is_hidden(),
            "toggled": info.toggled().map(|t| format!("{t:?}")),
            "bounds": info.bounds.map(rect),
        }))
    });
    Some(json!({
        "handle": h,
        "kind": tag,
        "core_kind": core_kind,
        "anchor": anchor,
        "box": drawn.map(rect),
        "visible_box": shown.map(|s| rect(s.visible)),
        "layout_box": layout.map(rect),
        "computed": computed,
        "matched_rules": rules.iter().map(rule_json).collect::<Vec<_>>(),
        "state": { "forced": state_names(forced), "own": state_names(own) },
        "accesskit": ak.unwrap_or(Value::Null),
    }))
}

impl Renderer {
    /// The nodes of the mounted cell `selector` picks ([`select_in`] under the cell's slot, or
    /// under a window story's window).
    pub fn select(&self, selector: &str) -> Vec<svl_handle> {
        match self.cell_root() {
            Some(slot) => select_in(self.backend(), slot, selector),
            None => Vec::new(),
        }
    }

    /// Mounts `cell` and describes the nodes `selector` picks (see the module docs).
    pub fn inspect(&mut self, cell: &Cell, selector: &str) -> Result<Value, Error> {
        let m = self.mount(cell)?;
        let picked = self.select(selector);
        if picked.is_empty() {
            return Err(Error::Request(format!("no node matches {selector:?} in {} (a selector is #anchor or a kind name)", cell.label())));
        }
        let window = self.harness().window();
        let backend = self.backend().clone();
        let a11y = self.harness().a11y().clone();
        let nodes: Vec<Value> = picked.into_iter().filter_map(|h| node_json(&backend, Some(window), h, Some(&a11y))).collect();
        Ok(json!({
            "cell": cell,
            "label": cell.label(),
            "selector": selector,
            "applied": m.applied,
            "warnings": m.warnings,
            "nodes": nodes,
        }))
    }
}
