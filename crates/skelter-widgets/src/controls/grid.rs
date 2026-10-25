//! `grid` (ported from svelters-egui `kinds/layouts.rs` and `layout.rs`): a CSS grid container laid
//! out by taffy. Children fill the cells row by row (auto-placement); rows are implicit (`auto`).
//! It paints its own CSS box.
//!
//! `grid-template-columns` takes the CSS subset [`parse_grid_template`] reads; a value outside it is
//! rejected and the prop keeps its value. The props follow the box props' precedence ([`Layout`]);
//! like every svelters-egui container the grid packs its tracks at the start (`justify-content:
//! start`) unless CSS says otherwise.

use crate::controls::box_::{ALIGN, Explicit, Layout};
use crate::kind_props;
use crate::paint::{self, PaintCx, Ua};
use crate::shim::core::{A11yCx, Kind, KindSpec, NameFrom, PropError, PropId, RecordCx, StyleCx, Value};
use skelter_base::accesskit;
use taffy::style_helpers::{auto, fit_content, fr, length, max_content, min_content, minmax, percent, repeat};
use taffy::{AlignContent, AlignItems, Display, GridTemplateComponent, LengthPercentage, MaxTrackSizingFunction, MinTrackSizingFunction, Style, TrackSizingFunction};

pub const JUSTIFY_ITEMS: &[&str] = &["start", "end", "center", "stretch"];

kind_props! {
    /// The grid's props.
    pub struct GridProps;
    /// The column tracks, CSS syntax: `auto`, `min-content`, `max-content`, `<n>fr`, `<n>px` (or a
    /// bare number: points), `<n>%`, `minmax(a, b)`, `fit-content(<len>)`, `repeat(<n>, tracks)`.
    /// Rows are implicit (`auto`). The default is a form: a label column and a field column.
    grid_template_columns "grid-template-columns": String = Str("auto 1fr") ; Str, Layout;
    /// Space between rows and between columns, in points (row-gap and column-gap override it).
    gap "gap": f64 = F64(0.0) ; F64, Layout;
    /// Space between rows in points; null to use `gap`.
    row_gap "row-gap": Option<f64> = None ; F64, Layout;
    /// Space between columns in points; null to use `gap`.
    column_gap "column-gap": Option<f64> = None ; F64, Layout;
    /// Inner spacing on all four sides, in points.
    padding "padding": f64 = F64(0.0) ; F64, Layout;
    /// Vertical alignment of each child in its cell.
    align_items "align-items": String = Enum("stretch") ; Enum(ALIGN), Layout;
    /// Horizontal alignment of each child in its cell.
    justify_items "justify-items": String = Enum("stretch") ; Enum(JUSTIFY_ITEMS), Layout;
}

pub static SPEC: KindSpec = KindSpec { tag: "grid", props: GridProps::SPECS, signals: &[], slots: &[], children: true, role: accesskit::Role::GenericContainer, name_from: NameFrom::None, text_prop: None, focusable: false };

/// A CSS grid container.
#[derive(Debug, Default)]
pub struct Grid {
    pub props: GridProps,
    explicit: Explicit,
}

impl Kind for Grid {
    fn spec() -> &'static KindSpec {
        &SPEC
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &SPEC
    }
    fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        if prop == P::grid_template_columns.id()
            && let Some(src) = v.as_str()
            && let Err(message) = parse_grid_template(src)
        {
            return Err(PropError::Invalid { prop: "grid-template-columns", message });
        }
        self.props.set(prop, v)?;
        self.explicit.mark(prop);
        Ok(())
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }

    fn layout_style(&self, cx: &StyleCx) -> Style {
        let mut s = cx.default_layout();
        let l = Layout { cx, explicit: self.explicit };
        let p = &self.props;
        l.display(&mut s, Display::Grid);
        l.gaps(&mut s, (P::gap.id(), p.gap), Some((P::row_gap.id(), p.row_gap)), Some((P::column_gap.id(), p.column_gap)));
        l.padding(&mut s, P::padding.id(), p.padding);
        l.align_items(&mut s, P::align_items.id(), &p.align_items);
        if !l.css_sets(&["justify-content"]) {
            s.justify_content = AlignContent::START;
        }
        if l.applies(&[P::grid_template_columns.id()], &["grid-template-columns"]) {
            s.grid_template_columns = parse_grid_template(&p.grid_template_columns).unwrap_or_default();
        }
        if l.applies(&[P::justify_items.id()], &["justify-items"]) {
            s.justify_items = match p.justify_items.as_str() {
                "start" => AlignItems::START,
                "end" => AlignItems::END,
                "center" => AlignItems::CENTER,
                _ => AlignItems::STRETCH,
            };
        }
        s
    }

    fn record(&self, cx: &mut RecordCx) {
        let host = crate::controls::common::host_view(&cx.style, Ua::NONE);
        let rect = cx.rect();
        let mut pc = PaintCx::of(cx);
        paint::paint_box(&mut pc, &host, rect);
        paint::paint_outline(&mut pc, &host, rect);
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::GenericContainer);
    }
}

// ---- grid-template-columns ----

/// Parses a `grid-template-columns` value (the CSS subset in the prop's doc). Empty: no explicit
/// columns (one implicit `auto` column).
pub fn parse_grid_template(src: &str) -> Result<Vec<GridTemplateComponent<String>>, String> {
    let mut out = Vec::new();
    for item in split_top(src, |c| c.is_ascii_whitespace())? {
        if let Some(inner) = call(&item, "repeat") {
            let parts = split_top(inner, |c| c == ',')?;
            let [count, tracks] = parts.as_slice() else { return Err(format!("`{item}`: repeat takes a count and tracks")) };
            let n: u16 = count.trim().parse().ok().filter(|n| *n > 0).ok_or_else(|| format!("`{item}`: the count must be a positive integer"))?;
            let tracks = split_top(tracks, |c| c.is_ascii_whitespace())?.iter().map(|t| track(t)).collect::<Result<Vec<_>, _>>()?;
            if tracks.is_empty() {
                return Err(format!("`{item}`: no tracks"));
            }
            out.push(repeat(n, tracks));
        } else {
            out.push(GridTemplateComponent::Single(track(&item)?));
        }
    }
    Ok(out)
}

/// `name(args)` → `args`.
fn call<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    s.strip_prefix(name)?.trim_start().strip_prefix('(')?.strip_suffix(')')
}

/// Splits at top-level separators (outside parentheses), dropping empty pieces.
fn split_top(s: &str, sep: impl Fn(char) -> bool) -> Result<Vec<String>, String> {
    let (mut out, mut cur, mut depth) = (Vec::new(), String::new(), 0i32);
    for c in s.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            return Err(format!("`{s}`: unbalanced parentheses"));
        }
        if depth == 0 && sep(c) {
            if !cur.trim().is_empty() {
                out.push(cur.trim().to_string());
            }
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    if depth != 0 {
        return Err(format!("`{s}`: unbalanced parentheses"));
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    Ok(out)
}

enum Breadth {
    Points(f32),
    Percent(f32),
    Fr(f32),
}

fn breadth(t: &str) -> Option<Breadth> {
    let num = |s: &str| s.trim().parse::<f32>().ok().filter(|v| v.is_finite() && *v >= 0.0);
    if let Some(v) = t.strip_suffix("fr") {
        num(v).map(Breadth::Fr)
    } else if let Some(v) = t.strip_suffix('%') {
        num(v).map(Breadth::Percent)
    } else {
        num(t.strip_suffix("px").unwrap_or(t)).map(Breadth::Points)
    }
}

fn track(t: &str) -> Result<TrackSizingFunction, String> {
    let bad = || format!("`{t}` is not a grid track (auto, min-content, max-content, <n>fr, <n>px, <n>%, minmax(), fit-content())");
    Ok(match t {
        "auto" => auto(),
        "min-content" => min_content(),
        "max-content" => max_content(),
        _ => {
            if let Some(inner) = call(t, "minmax") {
                let parts = split_top(inner, |c| c == ',')?;
                let [a, b] = parts.as_slice() else { return Err(bad()) };
                minmax(min_track(a).ok_or_else(bad)?, max_track(b).ok_or_else(bad)?)
            } else if let Some(inner) = call(t, "fit-content") {
                match breadth(inner.trim()).ok_or_else(bad)? {
                    Breadth::Points(v) => fit_content(LengthPercentage::length(v)),
                    Breadth::Percent(v) => fit_content(LengthPercentage::percent(v / 100.0)),
                    Breadth::Fr(_) => return Err(bad()),
                }
            } else {
                match breadth(t).ok_or_else(bad)? {
                    Breadth::Points(v) => length(v),
                    Breadth::Percent(v) => percent(v / 100.0),
                    Breadth::Fr(v) => fr(v),
                }
            }
        }
    })
}

fn min_track(t: &str) -> Option<MinTrackSizingFunction> {
    Some(match t.trim() {
        "auto" => auto(),
        "min-content" => min_content(),
        "max-content" => max_content(),
        t => match breadth(t)? {
            Breadth::Points(v) => length(v),
            Breadth::Percent(v) => percent(v / 100.0),
            Breadth::Fr(_) => return None,
        },
    })
}

fn max_track(t: &str) -> Option<MaxTrackSizingFunction> {
    Some(match t.trim() {
        "auto" => auto(),
        "min-content" => min_content(),
        "max-content" => max_content(),
        t => match breadth(t)? {
            Breadth::Points(v) => length(v),
            Breadth::Percent(v) => percent(v / 100.0),
            Breadth::Fr(v) => fr(v),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit;
    use crate::interact::StateBits;
    use skelter_base::change::ChangeClass;
    use skelter_base::geom::Size;

    fn set(k: &mut Grid, name: &str, v: Value) -> Result<(), PropError> {
        let id = SPEC.prop(name).unwrap();
        k.set_prop(id, &v)
    }

    #[test]
    fn spec() {
        let s = Grid::spec();
        assert_eq!(s.tag, "grid");
        assert!(s.children && s.signals.is_empty());
        assert!(s.props.iter().all(|p| p.change == ChangeClass::Layout));
        assert_eq!(s.prop_spec(s.prop("grid-template-columns").unwrap()).unwrap().default, crate::shim::core::Default::Str("auto 1fr"));
        assert!(s.prop("justify-items").is_some() && s.prop("row-gap").is_some());
    }

    #[test]
    fn parses_the_track_subset() {
        for ok in ["", "auto", "repeat(3, 1fr)", "minmax(40px, 1fr) 100", "fit-content(50%) max-content min-content 25%"] {
            assert!(parse_grid_template(ok).is_ok(), "{ok}");
        }
        for bad in ["1fx", "repeat(0, 1fr)", "minmax(1fr, 2fr)", "(", "fit-content(1fr)"] {
            assert!(parse_grid_template(bad).is_err(), "{bad}");
        }
        assert_eq!(parse_grid_template("auto 1fr").unwrap(), vec![GridTemplateComponent::Single(auto()), GridTemplateComponent::Single(fr(1.0))]);
        assert_eq!(parse_grid_template("100 25%").unwrap(), vec![GridTemplateComponent::Single(length(100.0)), GridTemplateComponent::Single(percent(0.25))]);
        assert_eq!(parse_grid_template("repeat(2, 10px)").unwrap(), vec![repeat(2u16, vec![length(10.0)])]);
    }

    #[test]
    fn rejects_a_bad_template_and_keeps_its_value() {
        let mut k = Grid::default();
        set(&mut k, "grid-template-columns", Value::str("1fr 1fr")).unwrap();
        assert!(matches!(set(&mut k, "grid-template-columns", Value::str("1fr bogus")), Err(PropError::Invalid { prop: "grid-template-columns", .. })));
        assert_eq!(k.get_prop(SPEC.prop("grid-template-columns").unwrap()), Some(Value::str("1fr 1fr")));
    }

    #[test]
    fn layout_style_defaults_to_a_form_grid() {
        let k = Grid::default();
        let s = testkit::layout_style(&k, None, StateBits::NONE);
        assert_eq!(s.display, Display::Grid);
        assert_eq!(s.grid_template_columns, vec![GridTemplateComponent::Single(auto()), GridTemplateComponent::Single(fr(1.0))]);
        assert_eq!(s.align_items, AlignItems::STRETCH);
        assert_eq!(s.justify_items, AlignItems::STRETCH);
        assert_eq!(s.justify_content, AlignContent::START);
    }

    #[test]
    fn layout_style_from_props() {
        let mut k = Grid::default();
        set(&mut k, "grid-template-columns", Value::str("repeat(3, 1fr)")).unwrap();
        set(&mut k, "gap", Value::F64(6.0)).unwrap();
        set(&mut k, "row-gap", Value::F64(2.0)).unwrap();
        set(&mut k, "padding", Value::F64(3.0)).unwrap();
        set(&mut k, "align-items", Value::str("center")).unwrap();
        set(&mut k, "justify-items", Value::str("end")).unwrap();
        let s = testkit::layout_style(&k, None, StateBits::NONE);
        assert_eq!(s.grid_template_columns, vec![repeat(3u16, vec![fr(1.0)])]);
        assert_eq!(s.gap.width, LengthPercentage::length(6.0), "column gap: `gap`");
        assert_eq!(s.gap.height, LengthPercentage::length(2.0), "row gap: `row-gap`");
        assert_eq!(s.padding, taffy::Rect::length(3.0));
        assert_eq!(s.align_items, AlignItems::CENTER);
        assert_eq!(s.justify_items, AlignItems::END);
    }

    #[test]
    fn css_template_beats_the_default() {
        let css = testkit::css("grid-template-columns: 1fr 2fr; display: flex");
        let mut k = Grid::default();
        let s = testkit::layout_style(&k, Some(&css), StateBits::NONE);
        // the CSS rule decides: the base skelter-core maps holds it, the prop default stays out
        assert_ne!(s.grid_template_columns, vec![GridTemplateComponent::Single(auto()), GridTemplateComponent::Single(fr(1.0))]);
        assert_ne!(s.display, Display::Grid, "CSS `display` wins over the kind's grid");
        set(&mut k, "grid-template-columns", Value::str("100 1fr")).unwrap();
        let s = testkit::layout_style(&k, Some(&css), StateBits::NONE);
        assert_eq!(s.grid_template_columns, vec![GridTemplateComponent::Single(length(100.0)), GridTemplateComponent::Single(fr(1.0))]);
    }

    #[test]
    fn records_its_css_box() {
        let k = Grid::default();
        let size = Size::new(120.0, 60.0);
        assert!(testkit::record(&k, size, StateBits::NONE).0.items.is_empty());
        let css = testkit::css("border-width: 1px; border-color: #808080");
        let (list, _) = testkit::record_css(&k, size, StateBits::NONE, &css);
        assert_eq!(testkit::kinds(&list), ["quad"]);
        let skelter_base::display::Prim::Quad(q) = &list.items[0] else { unreachable!() };
        assert_eq!(q.border_widths.top, 1.0);
    }
}
