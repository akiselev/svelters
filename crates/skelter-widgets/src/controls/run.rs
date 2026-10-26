//! Inline rich text (RT2, WIDGET-PARITY Q4): the run kinds `span`, `b`, `i`, `code` and `a`,
//! children of a `label` (or of another run). They take no space and draw nothing themselves: the
//! label folds them, in document order, into its paragraph ([`fold`] → its `runs` records,
//! `controls::rich`), so `<label>a <b>b</b></label>` is one paragraph with a bold run.
//!
//! - **The markup is the template.** The compiler lowers a label's children (text, `{expressions}`,
//!   run elements, `{#if}`/`{#each}` of those) into run nodes, and its bare text into anonymous
//!   `span`s.
//! - **Formatting by tag.** `b` is bold, `i` italic, `code` monospace, `a` a link (its `href`); a run
//!   inherits its enclosing runs' formatting, and a `font-weight` of 600 or more is bold.
//! - **Links.** A click on an `a` run's glyphs is the label's `link-clicked` (with the run's index and
//!   `href`) and `open-url`.
//!
//! **Styled by CSS.** skelter-core restyles the runs, so `label span.warn` and `label a:hover` match
//! them; their computed colour (where it differs from the label's), weight (600 or more is bold) and
//! `font-style: italic` fold into the records. Per-run size and background are not folded (the
//! records have no field for them).

use crate::interact::StateBits;
use crate::kind_props;
use crate::shim::core::{A11yCx, HitCx, HitResult, Kind, KindSpec, NameFrom, PropError, PropId, RecordCx, SignalSpec, StyleCx, Value};
use skelter_base::accesskit;
use skelter_base::geom::Point;
use std::sync::Arc;

/// The run tags, in kind order.
pub const TAGS: &[&str] = &["span", "b", "i", "code", "a"];

kind_props! {
    /// A run's props.
    pub struct RunProps;
    /// The run's own text, before its child runs (its text children fold into it).
    text "text": String = Str("") ; Str, Layout;
    /// CSS font weight of this run and the runs in it; 0 inherits. 600 or more is bold.
    font_weight "font-weight": i64 = I64(0) ; I64, Layout;
    /// The link target of an `a` run (other runs ignore it).
    href "href": String = Str("") ; Str, Paint;
}

/// The `a` run's signal (delivered as the label's `link-clicked`).
pub const LINK_SIGNALS: &[SignalSpec] = &[SignalSpec::new("clicked", &[], "The link was activated.")];

macro_rules! run_kind {
    ($(#[$m:meta])* $name:ident, $spec:ident, $tag:literal, $role:expr, $signals:expr) => {
        $(#[$m])*
        #[derive(Debug, Default)]
        pub struct $name {
            pub props: RunProps,
        }

        pub static $spec: KindSpec = KindSpec { tag: $tag, props: RunProps::SPECS, signals: $signals, slots: &[], children: true, role: $role, name_from: NameFrom::Prop("text"), text_prop: Some("text"), focusable: false };

        impl Kind for $name {
            fn spec() -> &'static KindSpec {
                &$spec
            }
            fn kind_spec(&self) -> &'static KindSpec {
                &$spec
            }
            fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
                self.props.set(prop, v)
            }
            fn get_prop(&self, prop: PropId) -> Option<Value> {
                self.props.get(prop)
            }
            fn layout_style(&self, _cx: &StyleCx) -> taffy::Style {
                // part of the label's paragraph: no box of its own
                taffy::Style { display: taffy::Display::None, ..taffy::Style::default() }
            }
            fn record(&self, _cx: &mut RecordCx) {}
            fn hit_test(&self, _cx: &HitCx, _p: Point) -> HitResult {
                HitResult::Miss
            }
            fn a11y(&self, cx: &mut A11yCx) {
                a11y_of($tag, &self.props, cx.node, cx.state);
            }
        }
    };
}

run_kind!(
    /// `span`: an inline run of text, styled by CSS. Bare text in a rich label becomes anonymous spans.
    Span, SPAN, "span", accesskit::Role::TextRun, &[]
);
run_kind!(
    /// `b`: bold inline text.
    Bold, BOLD, "b", accesskit::Role::TextRun, &[]
);
run_kind!(
    /// `i`: italic inline text.
    Italic, ITALIC, "i", accesskit::Role::TextRun, &[]
);
run_kind!(
    /// `code`: inline code in the monospace font.
    Code, CODE, "code", accesskit::Role::TextRun, &[]
);
run_kind!(
    /// `a`: a link run (`href`).
    Anchor, ANCHOR, "a", accesskit::Role::Link, LINK_SIGNALS
);

/// A run's AccessKit node: `a` is a link named by its text (with its URL); the other runs are read as
/// part of their label's text.
fn a11y_of(tag: &str, p: &RunProps, node: &mut accesskit::Node, _state: StateBits) {
    if tag == "a" {
        node.set_role(accesskit::Role::Link);
        if !p.text.is_empty() {
            node.set_label(p.text.clone());
        }
        if !p.href.is_empty() {
            node.set_url(p.href.clone());
        }
    } else {
        node.set_role(accesskit::Role::TextRun);
        node.set_hidden();
    }
}

/// One run node as the label sees it: its tag, own text, `href`, `font-weight`, and child runs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunNode {
    pub tag: String,
    pub text: String,
    pub href: String,
    pub weight: i64,
    /// `font-style: italic` from its computed style.
    pub italic: bool,
    /// Its computed colour, when it differs from the label's.
    pub color: Option<skelter_base::color::Color>,
    pub children: Vec<RunNode>,
}

#[derive(Clone, Copy, Default)]
struct Fmt<'a> {
    bold: bool,
    italic: bool,
    mono: bool,
    href: Option<&'a str>,
    color: Option<skelter_base::color::Color>,
}

/// The label's `runs` records for `nodes` (its run children, in order): each run's own text, then its
/// children's, formatted by tag and inherited down. Runs with no text add nothing; non-run tags are
/// skipped with their subtree.
pub fn fold(nodes: &[RunNode]) -> Vec<Value> {
    let mut out = Vec::new();
    for n in nodes {
        fold_one(n, Fmt::default(), &mut out);
    }
    out
}

fn fold_one<'a>(n: &'a RunNode, up: Fmt<'a>, out: &mut Vec<Value>) {
    if !TAGS.contains(&n.tag.as_str()) {
        return;
    }
    let f = Fmt {
        bold: up.bold || n.tag == "b" || n.weight >= 600,
        italic: up.italic || n.italic || n.tag == "i",
        mono: up.mono || n.tag == "code",
        href: if n.tag == "a" { Some(n.href.as_str()) } else { up.href },
        color: n.color.or(up.color),
    };
    if !n.text.is_empty() {
        let mut fields: Vec<(Arc<str>, Value)> = vec![(Arc::from("text"), Value::str(&n.text))];
        for (k, on) in [("b", f.bold), ("i", f.italic), ("code", f.mono)] {
            if on {
                fields.push((Arc::from(k), Value::Bool(true)));
            }
        }
        if let Some(h) = f.href {
            fields.push((Arc::from("href"), Value::str(h)));
        }
        if let Some(c) = f.color {
            fields.push((Arc::from("color"), Value::Color(c)));
        }
        out.push(Value::Record(fields));
    }
    for c in &n.children {
        fold_one(c, f, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::rich;
    use crate::controls::testkit;
    use skelter_base::geom::Size;

    fn node(tag: &str, text: &str, children: Vec<RunNode>) -> RunNode {
        RunNode { tag: tag.into(), text: text.into(), children, ..RunNode::default() }
    }

    #[test]
    fn specs() {
        for (spec, tag) in [(Span::spec(), "span"), (Bold::spec(), "b"), (Italic::spec(), "i"), (Code::spec(), "code"), (Anchor::spec(), "a")] {
            assert_eq!(spec.tag, tag);
            assert_eq!(spec.text_prop, Some("text"));
            assert!(spec.children && !spec.focusable);
        }
        assert_eq!(Anchor::spec().role, accesskit::Role::Link);
        assert!(Anchor::spec().signal("clicked").is_some());
    }

    #[test]
    fn runs_take_no_space_and_draw_nothing() {
        let b = Bold::default();
        let st = b.layout_style(&StyleCx { computed: None, state: StateBits::NONE, dark: true, base: None });
        assert_eq!(st.display, taffy::Display::None);
        assert!(testkit::record(&b, Size::new(10.0, 10.0), StateBits::NONE).0.is_empty());
    }

    #[test]
    fn folds_in_document_order_with_inherited_format() {
        // <label>a <b>b <i>c</i></b><a href="x">d</a></label>
        let kids = vec![node("span", "a ", vec![]), node("b", "b ", vec![node("i", "c", vec![])]), RunNode { href: "x".into(), ..node("a", "d", vec![]) }];
        let runs = fold(&kids);
        let r = rich::from_value("", &runs).unwrap();
        assert_eq!(r.plain, "a b cd");
        assert_eq!(r.runs.len(), 4);
        assert!(!r.runs[0].fmt.bold);
        assert!(r.runs[1].fmt.bold && !r.runs[1].fmt.italic);
        assert!(r.runs[2].fmt.bold && r.runs[2].fmt.italic, "an i inside a b is both");
        assert_eq!(r.runs[3].href.as_deref(), Some("x"));
    }

    #[test]
    fn css_colour_and_italic_fold() {
        let red = skelter_base::color::Color::rgba(1.0, 0.0, 0.0, 1.0);
        let warn = RunNode { color: Some(red), italic: true, ..node("span", "warn ", vec![node("b", "inner", vec![])]) };
        let r = rich::from_value("", &fold(&[warn])).unwrap();
        assert!(r.runs[0].fmt.italic && r.runs[0].fmt.color == Some(red));
        assert!(r.runs[1].fmt.bold && r.runs[1].fmt.color == Some(red), "the colour is inherited down");
    }

    #[test]
    fn weight_and_unknown_tags() {
        let heavy = RunNode { weight: 700, ..node("span", "w", vec![]) };
        let r = rich::from_value("", &fold(&[heavy, node("div", "skipped", vec![node("b", "too", vec![])])])).unwrap();
        assert_eq!(r.plain, "w");
        assert!(r.runs[0].fmt.bold);
    }

    #[test]
    fn a_label_folds_its_children() {
        use crate::controls::label::Label;
        let runs = fold(&[node("span", "a ", vec![]), node("b", "b", vec![])]);
        let mut l = Label::default();
        let mut text = crate::shim::testing::MonoShaper;
        let mut cx = crate::shim::core::PrepareCx { size: Size::new(200.0, 20.0), style: crate::shim::core::Styles { computed: None, parts: &[], children: &runs, state: StateBits::NONE, dark: true }, text: &mut text, emitted: Vec::new() };
        l.prepare(&mut cx);
        let r = l.rich().expect("the children make it rich");
        assert_eq!(r.plain, "a b");
        assert!(r.runs[1].fmt.bold);
    }

    #[test]
    fn link_a11y() {
        let mut a = Anchor::default();
        a.props.text = "docs".into();
        a.props.href = "https://x".into();
        let n = testkit::a11y(&a, StateBits::NONE, Size::new(0.0, 0.0));
        assert_eq!(n.role(), accesskit::Role::Link);
        assert_eq!(n.label(), Some("docs"));
        let s = testkit::a11y(&Span::default(), StateBits::NONE, Size::new(0.0, 0.0));
        assert!(s.is_hidden());
    }
}
