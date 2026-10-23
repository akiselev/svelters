//! U1 (plans/research/UTILITY-CSS.md §7, U1's acceptance tests): the utility-class resolver (E1),
//! the stylist's utility layer (E2) and `color-mix()` for `/NN` (E4), on the egui table.

mod common;

use common::Tree;
use std::collections::HashMap;
use svelters_style::utilities::{ROWS, SCALES};
use svelters_style::utility_table::{RowKind, ValueSource};
use svelters_style::*;

/// The built-in theme's token values (dark set) from the semantic spec, `--sv-` names without the
/// prefix.
fn token_values() -> HashMap<String, String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../metadata/semantics/egui.json");
    let spec: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).expect("egui.json")).expect("json");
    let tokens = spec["theme"]["tokens"].as_object().expect("theme.tokens");
    tokens
        .iter()
        .map(|(k, t)| {
            let v = t.get("value").or_else(|| t.get("values").map(|v| &v["dark"])).and_then(|v| v.as_str()).unwrap_or_else(|| panic!("{k}: no value"));
            (k.clone(), v.to_string())
        })
        .collect()
}

/// `var(--sv-x)` replaced by the token's value.
fn substitute(css: &str, tokens: &HashMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = css;
    while let Some(i) = rest.find("var(--sv-") {
        out.push_str(&rest[..i]);
        let after = &rest[i + "var(--sv-".len()..];
        let end = after.find(')').expect("var()");
        let name = &after[..end];
        out.push_str(tokens.get(name).unwrap_or_else(|| panic!("no token {name} in {css}")));
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The theme's tokens as a sheet (on every element, so no inheritance is needed).
fn token_sheet(tokens: &HashMap<String, String>) -> String {
    let mut s = String::from("* { ");
    for (k, v) in tokens {
        s.push_str(&format!("--sv-{k}: {v}; "));
    }
    s.push('}');
    s
}

fn add(s: &mut Stylist, origin: Origin, css: &str) {
    let sheet = Stylesheet::parse(css, &ParseOptions::default());
    assert!(sheet.diagnostics.is_empty(), "{css}: {:?}", sheet.diagnostics);
    let (_, d) = s.add_sheet(&sheet, origin);
    assert!(d.is_empty(), "{css}: {d:?}");
}

/// A stylist on the egui table with utilities on, the theme's layer order and its tokens.
fn stylist() -> Stylist {
    let mut s = Stylist::new(&tables::EGUI);
    s.set_utilities(UtilityConfig::enabled());
    add(&mut s, Origin::Theme, "@layer components, utilities;");
    add(&mut s, Origin::Theme, &token_sheet(&token_values()));
    s
}

fn atoms(classes: &[&str]) -> Vec<Atom> {
    classes.iter().map(|c| Atom::new(c)).collect()
}

fn ensure(s: &mut Stylist, classes: &[&str]) -> Vec<UtilityIssue> {
    s.ensure_utilities(&atoms(classes), &ParseOptions::default())
}

/// One `box` (index 1, under a window) carrying `classes`, with `state`.
fn tree(tag: &str, classes: &[&str], state: StateMask) -> Tree {
    let mut t = Tree::parse(&format!("window\n  {tag}"));
    t.nodes[1].classes = classes.iter().map(|c| c.to_string()).collect();
    t.nodes[1].state = state;
    t
}

fn style(s: &Stylist, t: &Tree) -> ComputedStyle {
    s.compute(t.el(1), None, None, &ComputeContext::default(), &mut MatchCaches::new())
}

fn padding(st: &ComputedStyle) -> f32 {
    match st.get("padding") {
        Some(ComputedValue::Insets(s)) => s[0].px,
        other => panic!("padding: {other:?}"),
    }
}

/// U0's planned check and U1's first acceptance test: every vocabulary row resolves for each of
/// its keywords, every default value of its scales and a sample of its open sources; the rule it
/// emits parses with no diagnostics and passes the egui table, and with the built-in theme's
/// token values substituted it type-checks too.
#[test]
fn every_vocabulary_row_resolves_and_parses() {
    let tokens = token_values();
    let builtin = UtilityTokens::builtin();
    let res = Resolver::new(&tables::EGUI, &builtin);
    let mut checked = 0;
    for r in ROWS.iter().filter(|r| !r.pending) {
        let mut classes: Vec<String> = Vec::new();
        match r.kind {
            RowKind::Fixed { .. } => classes.push(r.name.to_string()),
            RowKind::Namespace { props, values, keywords, negative } => {
                for k in keywords {
                    classes.push(format!("{}-{}", r.name, k.name));
                }
                for v in values {
                    let samples: Vec<String> = match v {
                        ValueSource::Scale(s) => SCALES.iter().find(|x| x.name == *s).unwrap().defaults.iter().map(|(n, _)| n.to_string()).collect(),
                        ValueSource::Fraction => vec!["1/2".into()],
                        ValueSource::IntPx | ValueSource::Number | ValueSource::Integer(_) => vec!["2".into()],
                        ValueSource::Percent => vec!["50".into()],
                        ValueSource::Arbitrary => vec![arbitrary_sample(props[0])],
                    };
                    for v in samples {
                        classes.push(if v.is_empty() { r.name.to_string() } else { format!("{}-{v}", r.name) });
                    }
                }
                if negative {
                    classes.push(format!("-{}-[4px]", r.name));
                }
            }
        }
        for c in classes {
            let u = match res.resolve(&c) {
                Resolved::Utility(u) => u,
                other => panic!("{c} (row {} rank {}): {other:?}", r.name, r.rank),
            };
            assert_eq!(u.row_rank, r.rank, "{c} resolved through another row: {u:?}");
            let css = u.css();
            for text in [css.clone(), substitute(&css, &tokens)] {
                let sheet = Stylesheet::parse(&text, &ParseOptions::default());
                assert!(sheet.diagnostics.is_empty(), "{c}: {text}: {:?}", sheet.diagnostics);
                assert_eq!(sheet.rules.len(), 1, "{c}: {text}");
                let mut st = Stylist::new(&tables::EGUI);
                let (_, d) = st.add_sheet(&sheet, Origin::Author);
                assert!(d.is_empty(), "{c}: {text}: {d:?}");
            }
            checked += 1;
        }
    }
    assert!(checked > 1000, "{checked} utilities checked");
}

/// A valid arbitrary value for the property (by name for the raw ones).
fn arbitrary_sample(prop: &str) -> String {
    let ty = tables::EGUI.get(prop).map(|p| p.ty);
    match (prop, ty) {
        ("grid-template-columns" | "grid-template-rows", _) => "[1fr_2fr]",
        ("grid-column" | "grid-row", _) => "[1_/_3]",
        ("line-height", _) => "[1.5]",
        ("font-family", _) => "['Inter',_sans-serif]",
        (_, Some(ValueType::Color)) => "[#123456]",
        (_, Some(ValueType::Number)) => "[0.5]",
        (_, Some(ValueType::FontWeight)) => "[600]",
        (_, Some(ValueType::Shadow)) => "[0_1px_2px_#000]",
        (_, Some(ValueType::TimeList)) => "[2s]",
        (_, Some(ValueType::EasingList)) => "[linear]",
        _ => "[3px]",
    }
    .to_string()
}

/// Variants stack left to right: `hover:part-thumb:` is the hovered host's thumb,
/// `part-thumb:hover:` the thumb's own state (for a paint part, the host's); media variants hoist.
#[test]
fn variant_stacking_order() {
    let mut s = stylist();
    let classes = ["hover:part-thumb:bg-accent", "part-thumb:hover:border-accent", "dark:hover:bg-surface"];
    assert!(ensure(&mut s, &classes).is_empty());
    // the rule of class `c`: its selector names the escaped class
    let sel = |c: &str| {
        let escaped = format!(".{}", cssparser::ToCss::to_css_string(&Atom::new(c)));
        (0..s.rule_count() as u32).map(RuleId).find(|r| s.rule(*r).selectors.slice().iter().any(|x| cssparser::ToCss::to_css_string(x).starts_with(&escaped)))
    };
    let text = |c: &str| cssparser::ToCss::to_css_string(&s.rule(sel(c).unwrap()).selectors);
    assert!(text(classes[0]).ends_with(":hover::part(thumb)"), "{}", text(classes[0]));
    assert!(text(classes[1]).ends_with("::part(thumb):hover"), "{}", text(classes[1]));
    assert_eq!(s.rule(sel(classes[2]).unwrap()).media.len(), 1);
    let thumb = PseudoElement::part("thumb");
    let mut caches = MatchCaches::new();
    for (state, n) in [(StateMask::NONE, 0), (StateMask::HOVERED, 2)] {
        let t = tree("slider", &classes, state);
        assert_eq!(s.matched_pseudo_rules(t.el(1), &thumb, &mut caches).len(), n, "{state:?}");
    }
    // dark:hover: needs both
    let t = tree("slider", &classes, StateMask::HOVERED);
    let bg = |s: &Stylist| StyleView::new(&style(s, &t)).color("background-color");
    assert_eq!(bg(&s), None);
    s.set_media(MediaEnv { color_scheme: ColorScheme::Dark, ..Default::default() });
    assert_eq!(bg(&s).map(|c| c.rgba8()), Some([0x1f, 0x1f, 0x1f, 255]));
    let t = tree("slider", &classes, StateMask::NONE);
    assert_eq!(StyleView::new(&style(&s, &t)).color("background-color"), None);
}

/// Colours are our roles (UQ4, §9: the user chose our tokens; VS Code key aliases are not utility
/// names, U0 note 7): `bg-surface` is `var(--sv-surface)`; `bg-editor` is a plain class (U4 suggests the role
/// from `theme.tokens[].vscode`).
#[test]
fn colours_are_theme_roles() {
    let builtin = UtilityTokens::builtin();
    let res = Resolver::new(&tables::EGUI, &builtin);
    let Resolved::Utility(u) = res.resolve("bg-surface") else { panic!() };
    assert_eq!(u.decls, [(Atom::new("background-color"), "var(--sv-surface)".to_string())]);
    assert_eq!(res.resolve("bg-editor"), Resolved::NotUtility);
    assert!(matches!(res.resolve("bg-surfce"), Resolved::Invalid(UtilityIssue { kind: IssueKind::UnknownValue, .. })));
    // an app's own tokens are values with no regeneration (`--sv-cad-panel`)
    let mut tokens = UtilityTokens::builtin();
    tokens.insert("--sv-cad-panel", "color");
    let Resolved::Utility(u) = Resolver::new(&tables::EGUI, &tokens).resolve("hover:bg-cad-panel") else { panic!() };
    assert_eq!(u.decls[0].1, "var(--sv-cad-panel)");
}

/// `/50` mixes the token's colour with transparent (E4), after `var()` substitution.
#[test]
fn opacity_modifier_mixes_the_colour() {
    let mut s = stylist();
    assert!(ensure(&mut s, &["bg-accent/50", "text-fg/[0.25]", "border-accent/(--a)"]).is_empty());
    let t = tree("box", &["bg-accent/50", "text-fg/[0.25]"], StateMask::NONE);
    let st = style(&s, &t);
    let v = StyleView::new(&st);
    assert_eq!(v.color("background-color").unwrap().rgba8(), [0x00, 0x78, 0xd4, 128]);
    assert_eq!(v.color("color").unwrap().rgba8()[3], 64);
}

/// Utilities sit in an author layer: above the theme, below a component's unlayered rule; `!`
/// forces them.
#[test]
fn cascade_position_and_important() {
    let mut s = stylist();
    add(&mut s, Origin::Theme, "box { padding: 1px }");
    add(&mut s, Origin::Author, ".own { padding: 3px }");
    assert!(ensure(&mut s, &["p-2", "p-2!"]).is_empty());
    let space2: f32 = token_values()["space-2"].trim_end_matches("px").parse().unwrap();
    assert_eq!(padding(&style(&s, &tree("box", &["p-2"], StateMask::NONE))), space2, "a utility beats the theme");
    assert_eq!(padding(&style(&s, &tree("box", &["own", "p-2"], StateMask::NONE))), 3.0, "a component rule beats a utility");
    assert_eq!(padding(&style(&s, &tree("box", &["own", "p-2!"], StateMask::NONE))), space2, "`!` beats it back");
}

/// Arbitrary values are checked against the property's type; a bad one generates nothing.
#[test]
fn arbitrary_values_are_type_checked() {
    let mut s = stylist();
    for bad in ["w-[red]", "bg-[13px]", "opacity-[2px]", "rounded-[#fff]", "[width:red]", "duration-[fast]"] {
        let issues = ensure(&mut s, &[bad]);
        assert!(matches!(issues.as_slice(), [UtilityIssue { kind: IssueKind::BadValue, .. }]), "{bad}: {issues:?}");
        assert!(!s.is_utility(bad), "{bad}");
    }
    for good in ["w-[13px]", "w-[calc(100%_-_8px)]", "w-[var(--x)]", "opacity-[0.3]", "[width:13px]", "duration-[2s]"] {
        assert!(ensure(&mut s, &[good]).is_empty(), "{good}");
        assert!(s.is_utility(good), "{good}");
    }
    // a colour literal is a lint: generated, reported (an error under strict_tokens)
    let issues = ensure(&mut s, &["bg-[#f00]"]);
    assert!(matches!(issues.as_slice(), [UtilityIssue { kind: IssueKind::ArbitraryColor, severity: Severity::Warning, .. }]), "{issues:?}");
    assert!(s.is_utility("bg-[#f00]"));
    s.set_utilities(UtilityConfig { strict_tokens: true, ..UtilityConfig::enabled() });
    let issues = ensure(&mut s, &["text-[#0f0]"]);
    assert!(matches!(issues.as_slice(), [UtilityIssue { kind: IssueKind::ArbitraryColor, severity: Severity::Error, .. }]), "{issues:?}");
}

/// The same class set discovered in two different orders gives identical cascades: the utility
/// layer's order is canonical (variant rank, vocabulary row, class name), not first appearance.
#[test]
fn order_independence() {
    let classes = ["p-2", "p-4", "hover:p-1", "dark:p-3", "md:px-5", "max-lg:px-6", "bg-accent", "bg-surface", "text-sm", "text-[13px]", "rounded", "rounded-md", "w-1/2", "w-full", "px-1", "pl-2"];
    let mut a = stylist();
    for c in classes {
        ensure(&mut a, &[c]);
    }
    let mut b = stylist();
    let rev: Vec<&str> = classes.iter().rev().copied().collect();
    ensure(&mut b, &rev[..5]);
    ensure(&mut b, &rev[5..]);
    let env = MediaEnv { color_scheme: ColorScheme::Dark, width: 1000.0, ..Default::default() };
    a.set_media(env);
    b.set_media(env);
    for state in [StateMask::NONE, StateMask::HOVERED] {
        let ta = tree("box", &classes, state);
        let tb = tree("box", &rev, state);
        let decls = |s: &Stylist, t: &Tree| s.cascaded(t.el(1), None, &mut MatchCaches::new()).iter().filter(|d| !d.is_custom()).map(|d| format!("{}: {}", d.name, d.value)).collect::<Vec<_>>();
        let (da, db) = (decls(&a, &ta), decls(&b, &tb));
        assert_eq!(da, db);
        assert!(da.len() >= 14, "{da:?}");
        // within one row, the class name breaks the tie: p-4 after p-2; shorthands before longhands
        let pos = |d: &[String], x: &str| d.iter().position(|v| v == x).unwrap_or_else(|| panic!("{x} in {d:?}"));
        assert!(pos(&da, "padding: var(--sv-space-2)") < pos(&da, "padding: var(--sv-space-4)"));
        assert!(pos(&da, "padding-left: var(--sv-space-1)") < pos(&da, "padding-left: var(--sv-space-2)"));
        assert!(pos(&da, "padding: var(--sv-space-4)") < pos(&da, "padding: var(--sv-space-3)"), "dark: after plain");
    }
}

/// E2: utilities are appended without rebuilding the index, however many batches.
#[test]
fn appending_never_rebuilds() {
    let mut s = stylist();
    let before = s.rebuild_count();
    for i in 0..1000 {
        assert!(ensure(&mut s, &[&format!("w-[{i}px]"), &format!("hover:h-[{i}px]")]).is_empty());
    }
    assert_eq!(s.rebuild_count(), before, "no rebuild while appending");
    assert_eq!(s.utility_stats().generated, 2000);
    assert_eq!(s.utility_stats().batches, 1000);
    let t = tree("box", &["w-[517px]", "hover:h-[3px]"], StateMask::HOVERED);
    let st = style(&s, &t);
    assert_eq!((StyleView::new(&st).px("width"), StyleView::new(&st).px("height")), (Some(517.0), Some(3.0)));
    // without the theme's `@layer` statement, the first utility rule declares the layer: one
    // rebuild, then none
    let mut s = Stylist::new(&tables::EGUI);
    s.set_utilities(UtilityConfig::enabled());
    add(&mut s, Origin::Author, ".x { width: 1px }");
    let before = s.rebuild_count();
    ensure(&mut s, &["w-2"]);
    ensure(&mut s, &["w-3", "h-2"]);
    assert_eq!(s.rebuild_count(), before + 1);
}

/// New rules join the invalidation maps: a utility class restyles its node, a state variant
/// registers its state.
#[test]
fn invalidation_knows_new_utilities() {
    let mut s = stylist();
    assert!(!s.invalidation().states_used().intersects(StateMask::CHECKED));
    ensure(&mut s, &["checked:bg-accent", "in-hover:p-2"]);
    assert_eq!(s.invalidation().class_changed("checked:bg-accent"), Reach::SELF);
    assert!(s.invalidation().states_used().intersects(StateMask::CHECKED));
    assert!(s.invalidation().state_changed(StateMask::NONE, StateMask::HOVERED).contains(Reach::DESCENDANTS));
}

/// Classes that aren't utilities (or are invalid) cost one lookup after the first time, and their
/// problems are reported once; new tokens give them another chance.
#[test]
fn negative_cache() {
    let mut s = stylist();
    let issues = ensure(&mut s, &["row", "my-widget", "bg-acent", "row"]);
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert_eq!(issues[0].kind, IssueKind::UnknownValue);
    assert_eq!(s.utility_stats().negative, 3);
    assert!(ensure(&mut s, &["row", "bg-acent"]).is_empty(), "reported once");
    assert_eq!(s.utility_stats().generated, 0);
    let mut tokens = UtilityTokens::builtin();
    tokens.insert("acent", "color");
    s.set_utility_tokens(tokens);
    assert!(ensure(&mut s, &["bg-acent"]).is_empty());
    assert!(s.is_utility("bg-acent"));
}

/// Past the high-water mark, only rules no node references are dropped, oldest first; a dropped
/// class is generated again on its next use.
#[test]
fn gc_drops_only_unreferenced_rules() {
    let mut s = stylist();
    s.set_utilities(UtilityConfig { high_water: 4, ..UtilityConfig::enabled() });
    let names: Vec<String> = (1..=10).map(|i| format!("w-[{i}px]")).collect();
    for n in &names {
        ensure(&mut s, &[n]);
    }
    s.retain_utilities(&atoms(&[&names[0], &names[1]]));
    s.retain_utilities(&atoms(&["not-a-utility"]));
    assert_eq!(s.collect_utilities(), 6);
    let live: Vec<bool> = names.iter().map(|n| s.is_utility(n)).collect();
    assert_eq!(live, [true, true, false, false, false, false, false, false, true, true]);
    assert_eq!(s.collect_utilities(), 0, "at the mark");
    let width = |s: &Stylist, c: &str| StyleView::new(&style(s, &tree("box", &[c], StateMask::NONE))).px("width");
    assert_eq!(width(&s, &names[2]), None, "a dropped rule no longer matches");
    assert_eq!(width(&s, &names[0]), Some(1.0));
    assert_eq!(width(&s, &names[9]), Some(10.0));
    ensure(&mut s, &[&names[2]]);
    assert_eq!(width(&s, &names[2]), Some(3.0), "generated again");
    // released ones become collectable
    s.release_utilities(&atoms(&[&names[0]]));
    assert_eq!(s.collect_utilities(), 1);
    assert!(!s.is_utility(&names[0]) && s.is_utility(&names[1]));
    assert_eq!(s.utility_stats().dropped, 7);
}

/// Opt-in (UQ8): off, nothing is generated; with a prefix, only prefixed classes are utilities.
#[test]
fn opt_in_and_prefix() {
    let mut s = Stylist::new(&tables::EGUI);
    assert!(!s.utility_config().enabled);
    assert!(ensure(&mut s, &["flex", "p-2"]).is_empty());
    assert_eq!(s.rule_count(), 0);
    s.set_utilities(UtilityConfig { prefix: Some("tw".into()), ..UtilityConfig::enabled() });
    ensure(&mut s, &["flex", "tw:flex", "tw:hover:p-2"]);
    assert!(!s.is_utility("flex") && s.is_utility("tw:flex") && s.is_utility("tw:hover:p-2"));
    let t = tree("box", &["flex"], StateMask::NONE);
    assert_eq!(StyleView::new(&style(&s, &t)).keyword("display"), None);
    let t = tree("box", &["tw:flex"], StateMask::NONE);
    assert_eq!(StyleView::new(&style(&s, &t)).keyword("display"), Some("flex"));
    // changing the prefix drops what was generated
    s.set_utilities(UtilityConfig::enabled());
    assert!(!s.is_utility("tw:flex"));
    assert_eq!(s.rule_count(), 0);
}

/// Part utilities go through the part index (U2 E3): another part's computation never tries them.
#[test]
fn part_utilities_use_the_part_index() {
    let mut s = stylist();
    ensure(&mut s, &["part-thumb:bg-accent", "part-track:bg-surface"]);
    let t = tree("slider", &["part-thumb:bg-accent", "part-track:bg-surface"], StateMask::NONE);
    assert_eq!(s.pseudo_candidates(t.el(1), &PseudoElement::part("thumb")).len(), 1);
    assert!(s.has_pseudo_rules(&PseudoElement::part("track")));
    assert!(!s.has_pseudo_rules(&PseudoElement::part("label")));
    let host = style(&s, &t);
    let thumb = s.compute_pseudo(t.el(1), &PseudoElement::part("thumb"), &host, &ComputeContext::default(), &mut MatchCaches::new());
    assert_eq!(StyleView::new(&thumb).color("background-color").map(|c| c.rgba8()), Some([0x00, 0x78, 0xd4, 255]));
}
