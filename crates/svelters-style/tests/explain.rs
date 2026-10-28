//! SV88: `Stylist::explain`, why a property has its value.

mod common;

use common::{El, Tree};
use std::collections::HashMap;
use svelters_style::*;

const fn prop(name: &'static str, ty: ValueType) -> Prop {
    Prop {
        name,
        via: Via::Setter,
        target: name,
        ty,
        values: &[],
        tags: &[],
        inline_only: false,
    }
}

static TABLE: Table = Table {
    toolkit: "test",
    dialect: Dialect::Css,
    setter_selectors: &[],
    units: &[("px", 1.0)],
    props: &[
        prop("color", ValueType::Color),
        prop("font-size", ValueType::Length),
        prop("gap", ValueType::Length),
        prop("width", ValueType::Length),
    ],
    ranges: &[],
};

const TREE: &str = "
window#root
  box#card.card
    label#title
    button#ok.primary
";

/// The stylist, the tree, inline blocks by id and every node's computed style.
struct Setup {
    stylist: Stylist,
    tree: Tree,
    inline: HashMap<usize, DeclBlock>,
    computed: HashMap<usize, ComputedStyle>,
}

impl StyleLookup<El<'_>> for Setup {
    fn inline(&self, el: El<'_>) -> Option<&DeclBlock> {
        self.inline.get(&el.ix)
    }
    fn computed(&self, el: El<'_>) -> Option<&ComputedStyle> {
        self.computed.get(&el.ix)
    }
}

fn setup(sheets: &[(Origin, &str)], inline: &[(&str, &str)]) -> Setup {
    let mut stylist = Stylist::new(&TABLE);
    for (origin, css) in sheets {
        let sheet = Stylesheet::parse(css, &ParseOptions::default());
        assert!(
            sheet.diagnostics.is_empty(),
            "{css}: {:?}",
            sheet.diagnostics
        );
        stylist.add_sheet(&sheet, *origin);
    }
    let tree = Tree::parse(TREE);
    let inline: HashMap<usize, DeclBlock> = inline
        .iter()
        .map(|(id, css)| (tree.by_id(id).ix, DeclBlock::parse_inline(css).0))
        .collect();
    // top-down: the outline lists parents first
    let mut computed: HashMap<usize, ComputedStyle> = HashMap::new();
    let mut caches = MatchCaches::new();
    for el in tree.all() {
        let parent = el.parent().and_then(|p| computed.get(&p.ix)).cloned();
        let s = stylist.compute(
            el,
            parent.as_ref(),
            inline.get(&el.ix),
            &ComputeContext::default(),
            &mut caches,
        );
        computed.insert(el.ix, s);
    }
    Setup {
        stylist,
        tree,
        inline,
        computed,
    }
}

impl Setup {
    fn explain(&self, id: &str, property: &str) -> Explanation<usize> {
        self.stylist
            .explain(self.tree.by_id(id), property, self)
            .map(&mut |e: El<'_>| e.ix)
    }
    fn ix(&self, id: &str) -> usize {
        self.tree.by_id(id).ix
    }
}

fn summary(e: &Explanation<usize>) -> Vec<(DeclOrigin, String, LossReason)> {
    e.beaten
        .iter()
        .map(|b| (b.decl.origin, b.decl.value.clone(), b.reason))
        .collect()
}

#[test]
fn theme_user_and_inline() {
    let s = setup(
        &[
            (
                Origin::Theme,
                "window { --blue: #0000ff; --accent: var(--blue) } button { color: var(--accent) } #ok { gap: 1px }",
            ),
            (Origin::Author, "button { gap: 2px }"),
            (
                Origin::User,
                "#card { --mine: #ff0000; --accent: var(--mine) } button.primary { color: var(--accent); gap: 3px }",
            ),
        ],
        &[("ok", "gap: 5px")],
    );

    // the user's rule beats the theme's
    let c = s.explain("ok", "color");
    let w = c.winner.as_ref().unwrap();
    assert_eq!(
        (w.origin, w.selectors.as_deref(), w.value.as_str()),
        (DeclOrigin::User, Some("button.primary"), "var(--accent)")
    );
    assert!(w.rule.is_some() && w.sheet.is_some());
    assert_eq!(
        summary(&c),
        [(
            DeclOrigin::Theme,
            "var(--accent)".into(),
            LossReason::Origin
        )]
    );
    assert_eq!(c.specified.as_deref(), Some("#ff0000"));
    assert!(matches!(c.computed, Some(ComputedValue::Color(_))));
    assert_eq!(c.inherited_from, None);
    // --accent comes from the user's #card rule, through --mine
    assert_eq!(c.vars.len(), 1);
    let accent = &c.vars[0];
    assert_eq!(
        (
            accent.name.as_str(),
            accent.declared_at,
            accent.value.as_deref()
        ),
        ("--accent", Some(s.ix("card")), Some("#ff0000"))
    );
    assert_eq!(
        accent.decl.as_ref().map(|d| (d.origin, d.value.as_str())),
        Some((DeclOrigin::User, "var(--mine)"))
    );
    assert_eq!(accent.uses.len(), 1);
    assert_eq!(
        (
            accent.uses[0].name.as_str(),
            accent.uses[0].declared_at,
            accent.uses[0].value.as_deref()
        ),
        ("--mine", Some(s.ix("card")), Some("#ff0000"))
    );
    assert!(accent.uses[0].uses.is_empty() && !accent.cycle);

    // inline beats the user's rule; everything else lost on origin, highest first
    let g = s.explain("ok", "gap");
    let w = g.winner.as_ref().unwrap();
    assert_eq!(
        (w.origin, w.rule, w.selectors.as_deref(), w.value.as_str()),
        (DeclOrigin::Inline, None, None, "5px")
    );
    assert_eq!(
        summary(&g),
        [
            (DeclOrigin::User, "3px".into(), LossReason::Origin),
            (DeclOrigin::Author, "2px".into(), LossReason::Origin),
            (DeclOrigin::Theme, "1px".into(), LossReason::Origin)
        ]
    );
    assert_eq!(g.specified.as_deref(), Some("5px"));

    // a custom property itself: the theme's on the root, overridden on #card by the user
    let a = s.explain("card", "--accent");
    assert_eq!(a.winner.as_ref().map(|d| d.origin), Some(DeclOrigin::User));
    assert_eq!(a.specified.as_deref(), Some("#ff0000"));
    let a = s.explain("ok", "--accent");
    assert!(a.winner.is_none());
    assert_eq!(a.inherited_from, Some(s.ix("card")));
}

#[test]
fn each_step_of_the_cascade() {
    let s = setup(
        &[(
            Origin::Author,
            "@layer a { #ok { width: 1px } } button { width: 2px } .primary { width: 3px } button.primary { width: 4px } button.primary { width: 5px } #ok { gap: 1px } button { gap: 2px !important }",
        )],
        &[],
    );
    let e = s.explain("ok", "width");
    assert_eq!(e.winner.as_ref().map(|d| d.value.as_str()), Some("5px"));
    let reasons: Vec<(String, LossReason)> = e
        .beaten
        .iter()
        .map(|b| (b.decl.value.clone(), b.reason))
        .collect();
    assert_eq!(
        reasons,
        [
            ("4px".into(), LossReason::Order),
            ("3px".into(), LossReason::Specificity),
            ("2px".into(), LossReason::Specificity),
            ("1px".into(), LossReason::Layer)
        ]
    );
    assert_eq!(e.beaten[3].decl.layer.as_deref(), Some("a"));
    let g = s.explain("ok", "gap");
    assert_eq!(
        g.winner.as_ref().map(|d| (d.value.as_str(), d.important)),
        Some(("2px", true))
    );
    assert_eq!(g.beaten[0].reason, LossReason::Importance);
}

#[test]
fn revert_inheritance_invalid_values_and_cycles() {
    // `revert` defers to the theme; the author rule it excluded lost to the rollback
    let s = setup(
        &[
            (Origin::Theme, "button { width: 1px }"),
            (
                Origin::Author,
                "button { width: 2px } #ok { width: revert }",
            ),
        ],
        &[],
    );
    let e = s.explain("ok", "width");
    assert_eq!(
        e.winner.as_ref().map(|d| (d.origin, d.value.as_str())),
        Some((DeclOrigin::Theme, "1px"))
    );
    assert_eq!(
        e.beaten.iter().map(|b| b.reason).collect::<Vec<_>>(),
        [LossReason::Rollback, LossReason::RolledBack]
    );

    // nothing on #title: color inherits from #card's rule
    let s = setup(
        &[(
            Origin::Author,
            "#card { color: #00ff00; --a: var(--b); --b: var(--a) } #title { width: var(--missing) } button { color: var(--a, #ff0000) }",
        )],
        &[],
    );
    let t = s.explain("title", "color");
    assert!(t.winner.is_none() && t.beaten.is_empty());
    assert_eq!(t.inherited_from, Some(s.ix("card")));
    // width doesn't inherit
    let w = s.explain("title", "width");
    assert_eq!(
        w.winner.as_ref().map(|d| d.value.as_str()),
        Some("var(--missing)")
    );
    assert_eq!(w.specified, None, "invalid at computed-value time");
    assert_eq!(
        (w.vars[0].declared_at, w.vars[0].value.as_deref()),
        (None, None)
    );
    assert_eq!(s.explain("card", "width").inherited_from, None);

    // --a and --b are a cycle: both guaranteed-invalid, the fallback applies
    let c = s.explain("ok", "color");
    assert_eq!(c.specified.as_deref(), Some("#ff0000"));
    let a = &c.vars[0];
    assert_eq!(
        (a.name.as_str(), a.declared_at, a.value.as_deref()),
        ("--a", Some(s.ix("card")), None)
    );
    let b = &a.uses[0];
    assert_eq!(b.name, "--b");
    assert!(b.uses[0].cycle && b.uses[0].name == "--a");
}
