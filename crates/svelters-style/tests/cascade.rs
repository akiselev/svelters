//! The cascade end to end: sheets → matching → cascade order → computed values.

mod common;

use common::Tree;
use svelters_style::*;

const fn prop(name: &'static str, via: Via, ty: ValueType, values: &'static [&'static str]) -> Prop {
    Prop { name, via, target: name, ty, values, tags: &[], inline_only: false }
}

/// A table with the properties the tests need (sorted by name).
static TABLE: Table = Table {
    toolkit: "test",
    // a toolkit CSS engine owns `via: css` properties (see `none_dialect_computes_css_props`)
    dialect: Dialect::Css,
    setter_selectors: &[],
    units: &[("px", 1.0)],
    props: &[
        prop("background", Via::Css, ValueType::Raw, &[]),
        prop("box-shadow", Via::Setter, ValueType::Shadow, &[]),
        prop("caret-color", Via::Setter, ValueType::Color, &["auto"]),
        prop("color", Via::Setter, ValueType::Color, &[]),
        prop("flex-direction", Via::Setter, ValueType::Enum, &["row", "column"]),
        prop("font-size", Via::Setter, ValueType::Length, &[]),
        prop("font-weight", Via::Setter, ValueType::FontWeight, &[]),
        prop("gap", Via::Setter, ValueType::Length, &[]),
        Prop { inline_only: true, ..prop("hexpand", Via::Setter, ValueType::Bool, &[]) },
        prop("padding", Via::Setter, ValueType::Insets, &[]),
        prop("width", Via::Setter, ValueType::Length, &[]),
    ],
    ranges: &[],
};

const TREE: &str = "
window#root
  box#card.card.dark
    label#title.title
    button#ok.primary :hovered
";

struct Setup {
    stylist: Stylist,
    tree: Tree,
}

fn setup(sheets: &[(Origin, &str)]) -> Setup {
    let mut stylist = Stylist::new(&TABLE);
    for (origin, css) in sheets {
        let sheet = Stylesheet::parse(css, &ParseOptions::default());
        assert!(sheet.diagnostics.is_empty(), "{css}: {:?}", sheet.diagnostics);
        let (_, diags) = stylist.add_sheet(&sheet, *origin);
        assert!(diags.is_empty(), "{css}: {diags:?}");
    }
    Setup { stylist, tree: Tree::parse(TREE) }
}

impl Setup {
    /// Computes the whole tree top-down and returns the style of `id`.
    fn style(&self, id: &str) -> ComputedStyle {
        self.style_with(id, None)
    }

    fn style_with(&self, id: &str, inline: Option<(&str, &DeclBlock)>) -> ComputedStyle {
        let target = self.tree.by_id(id);
        let mut chain = vec![target];
        while let Some(p) = chain.last().unwrap().parent() {
            chain.push(p);
        }
        let mut caches = MatchCaches::new();
        let mut parent: Option<ComputedStyle> = None;
        for el in chain.into_iter().rev() {
            let this_inline = inline.filter(|(i, _)| self.tree.nodes[el.ix].id.as_deref() == Some(*i)).map(|(_, b)| b);
            let s = self.stylist.compute(el, parent.as_ref(), this_inline, &ComputeContext::default(), &mut caches);
            parent = Some(s);
        }
        parent.unwrap()
    }
}

fn len(px: f32) -> ComputedValue {
    ComputedValue::Length(Length::px(px))
}

fn rgb(s: &ComputedStyle, name: &str) -> [u8; 4] {
    match s.get(name) {
        Some(ComputedValue::Color(c)) => c.rgba8(),
        other => panic!("{name}: {other:?}"),
    }
}

#[test]
fn specificity_then_source_order() {
    let s = setup(&[(Origin::Author, ".primary { gap: 1px } button { gap: 2px; width: 5px } #ok { width: 6px } button { width: 7px }")]);
    let ok = s.style("ok");
    assert_eq!(ok.get("gap"), Some(&len(1.0)));
    assert_eq!(ok.get("width"), Some(&len(6.0)));
    let s = setup(&[(Origin::Author, "button { gap: 1px } button { gap: 2px }")]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(2.0)));
}

#[test]
fn origins_and_important() {
    // theme < author for normal declarations
    let s = setup(&[(Origin::Theme, "#ok { gap: 1px }"), (Origin::Author, "button { gap: 2px }")]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(2.0)));
    // !important beats specificity; theme !important beats author !important
    let s = setup(&[(Origin::Author, "#ok { gap: 1px } button { gap: 2px !important }")]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(2.0)));
    let s = setup(&[(Origin::Theme, "button { gap: 3px !important }"), (Origin::Author, "#ok { gap: 4px !important }")]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(3.0)));
}

#[test]
fn inline_blocks() {
    let s = setup(&[(Origin::Author, "#ok { gap: 1px; width: 1px !important }")]);
    let (inline, d) = DeclBlock::parse_inline("gap: 9px; width: 9px; hexpand: true");
    assert!(d.is_empty());
    let ok = s.style_with("ok", Some(("ok", &inline)));
    assert_eq!(ok.get("gap"), Some(&len(9.0)));
    assert_eq!(ok.get("width"), Some(&len(1.0)), "an !important rule beats a normal inline declaration");
    assert_eq!(ok.get("hexpand"), Some(&ComputedValue::Bool(true)));
    let (inline, _) = DeclBlock::parse_inline("width: 8px !important");
    assert_eq!(s.style_with("ok", Some(("ok", &inline))).get("width"), Some(&len(8.0)));
}

#[test]
fn inline_only_props_are_rejected_in_rules() {
    let mut stylist = Stylist::new(&TABLE);
    let (_, diags) = stylist.add_sheet(&Stylesheet::parse("a { hexpand: true }", &ParseOptions::default()), Origin::Author);
    assert!(diags[0].message.contains("style="), "{diags:?}");
}

#[test]
fn layers() {
    // unlayered beats layered; later layers beat earlier ones, whatever the specificity
    let s = setup(&[(Origin::Author, "@layer base, ui; @layer ui { button { gap: 2px } } @layer base { #ok { gap: 1px } }")]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(2.0)));
    let s = setup(&[(Origin::Author, "@layer ui { #ok { gap: 2px } } button { gap: 3px }")]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(3.0)));
    // a layer's own rules beat its sublayers
    let s = setup(&[(Origin::Author, "@layer ui { @layer inner { #ok { gap: 1px } } button { gap: 2px } }")]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(2.0)));
    // !important reverses the layer order
    let s = setup(&[(Origin::Author, "@layer base, ui; @layer base { button { gap: 1px !important } } @layer ui { button { gap: 2px !important } } button { gap: 3px !important }")]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(1.0)));
}

/// `caret-color: auto | <color>`, inherited, initial `auto` (none computed): a colour inherits,
/// `auto` stays a keyword and inherits as one; skelter's and egui's tables carry it.
#[test]
fn caret_color_auto_or_a_colour_inherits() {
    let s = setup(&[(Origin::Author, "#card { caret-color: #00ff00 } button { caret-color: auto }")]);
    assert_eq!(s.style("root").get("caret-color"), None, "initial: auto, nothing computed");
    assert_eq!(rgb(&s.style("card"), "caret-color"), [0, 255, 0, 255]);
    assert_eq!(rgb(&s.style("title"), "caret-color"), [0, 255, 0, 255], "caret-color inherits");
    assert_eq!(s.style("ok").get("caret-color"), Some(&ComputedValue::Keyword("auto")));
    let s = setup(&[(Origin::Author, "#card { caret-color: AUTO; color: red } .title { caret-color: currentcolor }")]);
    assert_eq!(s.style("ok").get("caret-color"), Some(&ComputedValue::Keyword("auto")), "auto inherits as auto");
    assert_eq!(rgb(&s.style("title"), "caret-color"), [255, 0, 0, 255]);
    for t in [Table::for_toolkit("skelter").unwrap(), Table::for_toolkit("egui").unwrap()] {
        let p = t.props.iter().find(|p| p.name == "caret-color").expect("caret-color");
        assert_eq!((p.ty, p.values), (ValueType::Color, &["auto"][..]), "{}", t.toolkit);
        assert!(p.is_inherited());
    }
}

#[test]
fn inheritance_and_keywords() {
    let s = setup(&[(Origin::Author, "#card { color: red; gap: 4px; font-weight: 600 } .title { gap: inherit } button { color: initial; font-weight: bolder }")]);
    let title = s.style("title");
    assert_eq!(rgb(&title, "color"), [255, 0, 0, 255], "color inherits");
    assert_eq!(title.get("gap"), Some(&len(4.0)), "gap: inherit");
    let ok = s.style("ok");
    assert_eq!(ok.get("color"), None, "initial = the native default");
    assert_eq!(ok.get("font-weight"), Some(&ComputedValue::FontWeight(900)));
    assert_eq!(s.style("root").get("gap"), None);
}

#[test]
fn em_rem_and_percent_font_size() {
    let s = setup(&[(Origin::Author, "#card { font-size: 20px } .title { font-size: 150%; width: 2em; padding: 1em 1rem } button { width: calc(100% - 1em) }")]);
    let title = s.style("title");
    assert_eq!(title.get("font-size"), Some(&len(30.0)));
    assert_eq!(title.get("width"), Some(&len(60.0)));
    assert_eq!(title.get("padding"), Some(&ComputedValue::Insets([Length::px(30.0), Length::px(16.0), Length::px(30.0), Length::px(16.0)])));
    assert_eq!(s.style("ok").get("width"), Some(&ComputedValue::Length(Length { px: -20.0, percent: 100.0 })));
}

#[test]
fn custom_properties_and_var() {
    let s = setup(&[(
        Origin::Author,
        ":root { --accent: #00f; --pad: 2px }
         .card { --pad: 3px; --both: var(--pad) var(--pad); --a: var(--b); --b: var(--a); --c: var(--a, 5px) }
         .title { color: var(--accent); gap: calc(var(--pad) * 2); width: var(--a, 7px); padding: var(--both) }
         button { gap: var(--missing); width: var(--c); flex-direction: var(--dir, column) }",
    )]);
    let title = s.style("title");
    assert_eq!(rgb(&title, "color"), [0, 0, 255, 255]);
    assert_eq!(title.get("gap"), Some(&len(6.0)), "--pad is overridden by .card and inherited");
    assert_eq!(title.get("width"), Some(&len(7.0)), "a cycle is guaranteed-invalid, so the fallback applies");
    assert_eq!(title.get("padding"), Some(&ComputedValue::Insets([Length::px(3.0), Length::px(3.0), Length::px(3.0), Length::px(3.0)])));
    assert_eq!(title.custom("--a"), None);
    assert_eq!(title.custom("--both"), Some("3px   3px"));
    let ok = s.style("ok");
    assert_eq!(ok.get("gap"), None, "invalid at computed-value time = unset");
    assert_eq!(ok.get("width"), Some(&len(5.0)), "--c falls back past the invalid --a");
    assert_eq!(ok.get("flex-direction"), Some(&ComputedValue::Keyword("column")));
}

#[test]
fn via_css_props_are_not_computed() {
    let s = setup(&[(Origin::Author, "button { background: red; gap: 1px }")]);
    let ok = s.style("ok");
    assert_eq!(ok.get("background"), None);
    assert_eq!(ok.iter().map(|(p, _)| p.name).collect::<Vec<_>>(), ["gap"]);
}

#[test]
fn table_diagnostics() {
    let mut stylist = Stylist::new(Table::for_toolkit("egui").unwrap());
    let sheet = Stylesheet::parse("a { flex-direction: diagonal; halign: fill; nope: 1; gap: var(--x); --y: anything }", &ParseOptions::default());
    let (_, diags) = stylist.add_sheet(&sheet, Origin::Author);
    let msgs: Vec<String> = diags.iter().map(|d| d.to_string()).collect();
    assert_eq!(msgs.len(), 3, "{msgs:?}");
    assert!(msgs[0].starts_with("1:5: error: flex-direction: 'diagonal' is not one of"), "{msgs:?}");
    assert!(msgs[1].contains("`halign` is not an egui style property (it is one on gtk)"), "{msgs:?}");
    assert_eq!(diags[2].severity, Severity::Warning);
}

#[test]
fn media_switches_rules() {
    let mut s = setup(&[(Origin::Author, "button { gap: 1px } @media (prefers-color-scheme: dark) { button { gap: 2px } } @media (max-width: 500px) { .card { gap: 3px } }")]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(1.0)));
    let flipped = s.stylist.set_media(MediaEnv { color_scheme: ColorScheme::Dark, ..Default::default() });
    assert_eq!(flipped, [RuleId(1)]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(2.0)));
    let flipped = s.stylist.set_media(MediaEnv { color_scheme: ColorScheme::Dark, width: 400.0, ..Default::default() });
    assert_eq!(flipped, [RuleId(2)]);
    assert_eq!(s.style("card").get("gap"), Some(&len(3.0)));
}

#[test]
fn platform_media_follows_the_env() {
    // U2 E7: `-sv-platform` reads `MediaEnv::platform`, which the backend sets (and tests override)
    let mut s = setup(&[(
        Origin::Author,
        "button { gap: 1px } @media (-sv-platform: macos) { button { gap: 2px } } @media (-sv-platform: windows) { button { gap: 3px } }
         @media not (-sv-platform: linux) { .card { gap: 4px } }",
    )]);
    let flipped = s.stylist.set_media(MediaEnv { platform: Platform::Linux, ..Default::default() });
    assert_eq!(s.style("ok").get("gap"), Some(&len(1.0)));
    assert!(flipped.iter().all(|r| r.0 != 0));
    let flipped = s.stylist.set_media(MediaEnv { platform: Platform::Macos, ..Default::default() });
    assert_eq!(flipped, [RuleId(1), RuleId(3)]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(2.0)));
    assert_eq!(s.style("card").get("gap"), Some(&len(4.0)));
    let flipped = s.stylist.set_media(MediaEnv { platform: Platform::Windows, ..Default::default() });
    assert_eq!(flipped, [RuleId(1), RuleId(2)]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(3.0)));
    assert_eq!(MediaEnv::default().platform, Platform::current());
}

#[test]
fn matched_rules_in_cascade_order_and_removal() {
    let mut stylist = Stylist::new(&TABLE);
    let (a, _) = stylist.add_sheet(&Stylesheet::parse("#ok { gap: 1px } button, .primary.x, .primary { gap: 2px }", &ParseOptions::default()), Origin::Author);
    stylist.add_sheet(&Stylesheet::parse("* { gap: 3px }", &ParseOptions::default()), Origin::Theme);
    let tree = Tree::parse(TREE);
    let ok = tree.by_id("ok");
    let m = stylist.matched_rules(ok, &mut MatchCaches::new());
    let order: Vec<u32> = m.iter().map(|r| r.rule.0).collect();
    assert_eq!(order, [2, 1, 0], "theme first, then by specificity");
    assert_eq!(m[1].specificity, 1 << 10, "the highest matching selector of the list (.primary)");
    stylist.remove_sheet(a);
    assert_eq!(stylist.rule_count(), 1);
}

#[test]
fn changed_since_lists_pushes() {
    let s = setup(&[(Origin::Author, "button { gap: 1px; width: 2px }")]);
    let before = s.style("ok");
    let s2 = setup(&[(Origin::Author, "button { gap: 1px; color: red }")]);
    let after = s2.style("ok");
    let changed: Vec<&str> = after.changed_since(&before).into_iter().map(|i| TABLE.props[i].name).collect();
    assert_eq!(changed, ["color", "width"]);
}

#[test]
fn egui_table_end_to_end() {
    let mut stylist = Stylist::new(&tables::EGUI);
    let sheet = Stylesheet::parse(".card { flex-direction: row; gap: calc(2px * 3); padding: 4px } .card > :hover { flex-grow: 1 }", &ParseOptions::default());
    let (_, diags) = stylist.add_sheet(&sheet, Origin::Author);
    assert!(diags.is_empty(), "{diags:?}");
    let tree = Tree::parse(TREE);
    let mut caches = MatchCaches::new();
    let card = stylist.compute(tree.by_id("card"), None, None, &ComputeContext::default(), &mut caches);
    assert_eq!(card.get("flex-direction"), Some(&ComputedValue::Keyword("row")));
    assert_eq!(card.get("gap"), Some(&len(6.0)));
    let ok = stylist.compute(tree.by_id("ok"), Some(&card), None, &ComputeContext::default(), &mut caches);
    assert_eq!(ok.get("flex-grow"), Some(&ComputedValue::Number(1.0)));
    assert_eq!(ok.get("gap"), None, "gap doesn't inherit");
}

#[test]
fn none_dialect_computes_css_props() {
    static NONE: Table = Table { dialect: Dialect::None, ..TABLE };
    let mut stylist = Stylist::new(&NONE);
    stylist.add_sheet(&Stylesheet::parse("button { background: red }", &ParseOptions::default()), Origin::Author);
    let tree = Tree::parse(TREE);
    let ok = stylist.compute(tree.by_id("ok"), None, None, &ComputeContext::default(), &mut MatchCaches::new());
    assert_eq!(ok.get("background"), Some(&ComputedValue::Raw("red".into())));
}

/// Regression (found by CS5e): without `font-size` in the table, `%` resolved against the
/// parent's font size instead of staying a percentage.
#[test]
fn percent_stays_percent_without_font_size() {
    static NO_FONT: Table = Table {
        props: &[prop("gap", Via::Setter, ValueType::Length, &[]), prop("width", Via::Setter, ValueType::Length, &[])],
        ..TABLE
    };
    let mut stylist = Stylist::new(&NO_FONT);
    stylist.add_sheet(&Stylesheet::parse("button { width: calc(50% - 20px); gap: 10% }", &ParseOptions::default()), Origin::Author);
    let tree = Tree::parse(TREE);
    let ok = stylist.compute(tree.by_id("ok"), None, None, &ComputeContext::default(), &mut MatchCaches::new());
    assert_eq!(ok.get("width"), Some(&ComputedValue::Length(Length { px: -20.0, percent: 50.0 })));
    assert_eq!(ok.get("gap"), Some(&ComputedValue::Length(Length::percent(10.0))));
}

/// CS5g: `box-shadow` is typed: `em` from the element's font, `currentcolor` (omitted) from its
/// inherited `color`, `var()` inside the list; a bad value is a table error at its position (on
/// AppKit too: its decl dialect types `box-shadow` as a shadow).
#[test]
fn shadows_compute_and_check() {
    let s = setup(&[(Origin::Author, ".card { color: red; font-size: 10px } button { --c: blue; box-shadow: 0 1em 2px var(--c), inset 1px 1px }")]);
    let ok = s.style("ok");
    let Some(ComputedValue::Shadows(list)) = ok.get("box-shadow") else { panic!("{:?}", ok.get("box-shadow")) };
    assert_eq!(list.len(), 2);
    assert_eq!((list[0].y, list[0].blur, list[0].color.unwrap().rgba8()), (10.0, 2.0, [0, 0, 255, 255]));
    assert!(list[1].inset);
    assert_eq!(list[1].color.unwrap().rgba8(), [255, 0, 0, 255], "currentcolor");
    assert_eq!(s.style("card").get("box-shadow"), None, "box-shadow doesn't inherit");

    let mut stylist = Stylist::new(Table::for_toolkit("appkit").unwrap());
    let sheet = Stylesheet::parse("a {\n  box-shadow: 1px 2px -3px red;\n}", &ParseOptions::default());
    let (_, diags) = stylist.add_sheet(&sheet, Origin::Author);
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!((diags[0].severity, diags[0].line, diags[0].column), (Severity::Error, 2, 3), "{diags:?}");
    assert!(diags[0].message.contains("negative"), "{diags:?}");
}

/// SV70: the user origin ranks above author rules and below inline declarations; its
/// `!important` is top.
#[test]
fn user_origin_ranks() {
    // user normal beats author normal, whatever the specificity
    let s = setup(&[(Origin::Author, "#ok { gap: 1px }"), (Origin::User, "button { gap: 2px }")]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(2.0)));
    // user rules registered before the author's still win
    let s = setup(&[(Origin::User, "button { gap: 2px }"), (Origin::Author, "#ok { gap: 1px }")]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(2.0)));
    // inline normal beats user normal
    let (inline, _) = DeclBlock::parse_inline("gap: 9px");
    assert_eq!(s.style_with("ok", Some(("ok", &inline))).get("gap"), Some(&len(9.0)));
    // author !important beats user normal
    let s = setup(&[(Origin::Author, "button { gap: 1px !important }"), (Origin::User, "#ok { gap: 2px }")]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(1.0)));
    // user !important beats theme, author and inline !important
    let s = setup(&[(Origin::Theme, "#ok { gap: 3px !important }"), (Origin::User, "button { gap: 4px !important }"), (Origin::Author, "#ok { gap: 5px !important }")]);
    let (inline, _) = DeclBlock::parse_inline("gap: 6px !important");
    assert_eq!(s.style_with("ok", Some(("ok", &inline))).get("gap"), Some(&len(4.0)));
    // matched rules list user rules after author rules
    let s = setup(&[(Origin::User, "button { gap: 2px }"), (Origin::Author, "#ok { gap: 1px }")]);
    let el = s.tree.by_id("ok");
    let m = s.stylist.matched_rules(el, &mut MatchCaches::new());
    assert_eq!(m.len(), 2);
    assert_eq!(s.stylist.rule(m[1].rule).block.decls[0].value.as_ref(), "2px");
}

/// SV70: `revert` rolls back to the origin below; `revert-layer` to the layer below.
#[test]
fn revert_and_revert_layer() {
    let theme = (Origin::Theme, "button { gap: 1px; color: red }");
    // an author revert falls to the theme
    let s = setup(&[theme, (Origin::Author, "button { gap: 2px } #ok { gap: revert }")]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(1.0)));
    // a user revert falls to the author rules (not to the theme)
    let s = setup(&[theme, (Origin::Author, "button { gap: 2px }"), (Origin::User, "#ok { gap: revert }")]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(2.0)));
    // a user revert of a shorthand-free property with no author rule falls to the theme
    let s = setup(&[theme, (Origin::User, "#ok { color: revert }")]);
    assert_eq!(rgb(&s.style("ok"), "color"), [255, 0, 0, 255]);
    // an inline revert is author origin: it skips the author rules
    let s = setup(&[theme, (Origin::Author, "#ok { gap: 2px }")]);
    let (inline, _) = DeclBlock::parse_inline("gap: revert");
    assert_eq!(s.style_with("ok", Some(("ok", &inline))).get("gap"), Some(&len(1.0)));
    // a theme revert is `unset`: gap isn't inherited, so it is gone
    let s = setup(&[(Origin::Theme, "button { gap: 1px } #ok { gap: revert }")]);
    assert_eq!(s.style("ok").get("gap"), None);
    // revert-layer: to the earlier layer of the same origin
    let s = setup(&[(Origin::Author, "@layer a, b; @layer a { button { gap: 2px } } @layer b { #ok { gap: 3px } } #ok { gap: revert-layer }")]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(3.0)));
    let s = setup(&[(Origin::Author, "@layer a, b; @layer a { button { gap: 2px } } @layer b { #ok { gap: revert-layer } }")]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(2.0)));
    // from the lowest layer on to the origin below
    let s = setup(&[theme, (Origin::Author, "@layer a { #ok { gap: revert-layer } }")]);
    assert_eq!(s.style("ok").get("gap"), Some(&len(1.0)));
    // a user revert of padding and of a custom property falls to the author's values
    let base = [(Origin::Theme, "button { padding: 1px; --x: 4px }"), (Origin::Author, "button { padding: 2px; --x: 5px }")];
    let author = setup(&base).style("ok");
    let s = setup(&[base[0], base[1], (Origin::User, "#ok { padding: revert; --x: revert }")]);
    let ok = s.style("ok");
    assert!(ok.get("padding").is_some());
    assert_eq!(ok.get("padding"), author.get("padding"));
    assert_eq!(ok.custom("--x"), Some("5px"));
    let s = setup(&[(Origin::Theme, "button { --x: 4px }"), (Origin::Author, "#ok { --x: revert }")]);
    assert_eq!(s.style("ok").custom("--x"), Some("4px"));
}
