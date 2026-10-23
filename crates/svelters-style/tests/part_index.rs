//! U2 E3 (UTILITY-CSS §5): the part-rule index. Rules ending in a pseudo-element are bucketed by
//! part name (and `::placeholder`/`::selection`), then by the host compound's id, class or tag, so a
//! part's computation only tries the rules that can match it.
//!
//! The bench is ignored; run it in release:
//! `tools/dev/cargo-slot cargo nextest run --cargo-profile release -p svelters-style --test part_index --run-ignored only --no-capture`.

mod common;

use common::Tree;
use std::time::Instant;
use svelters_style::*;

const PARTS: usize = 30;
const HOSTS: usize = 10;

/// `PARTS × HOSTS` part rules on host classes, plus a few on tags and no host.
fn big_sheet() -> String {
    let mut css = String::new();
    for p in 0..PARTS {
        for h in 0..HOSTS {
            css.push_str(&format!(".host{h}::part(p{p}) {{ color: rgb({p}, {h}, 0) }}\n"));
        }
        css.push_str(&format!("slider::part(p{p}):hover {{ opacity: 0.5 }}\n"));
    }
    css.push_str("::part(p7) { letter-spacing: 1px }\n");
    css
}

fn stylist(css: &str) -> Stylist {
    let mut s = Stylist::new(&tables::EGUI);
    let sheet = Stylesheet::parse(css, &ParseOptions::default());
    assert!(sheet.diagnostics.is_empty(), "{:?}", sheet.diagnostics);
    let (_, diags) = s.add_sheet(&sheet, Origin::Author);
    assert!(diags.is_empty(), "{diags:?}");
    s
}

#[test]
#[ignore = "measurement: run in release"]
fn part_index_bench() {
    let s = stylist(&big_sheet());
    let tree = Tree::parse("window\n  slider#a.host3 :hovered\n  box#b.other");
    let cx = ComputeContext::default();
    let mut caches = MatchCaches::new();
    let a = tree.by_id("a");
    let b = tree.by_id("b");
    let host = s.compute(a, None, None, &cx, &mut caches);
    let pe = PseudoElement::part("p7");
    let miss = PseudoElement::part("nope");
    // sanity: the bucket finds what the scan found
    let st = s.compute_pseudo(a, &pe, &host, &cx, &mut caches);
    assert_eq!(st.get("color"), Some(&ComputedValue::Color(Color { r: 7.0 / 255.0, g: 3.0 / 255.0, b: 0.0, a: 1.0 })));
    assert_eq!(st.get("opacity"), Some(&ComputedValue::Number(0.5)));
    let median = |f: &mut dyn FnMut()| {
        let mut v: Vec<f64> = (0..301)
            .map(|_| {
                let t = Instant::now();
                for _ in 0..100 {
                    f();
                }
                t.elapsed().as_secs_f64() * 1e6 / 100.0
            })
            .collect();
        v.sort_by(f64::total_cmp);
        v[150]
    };
    let rules = s.rule_count();
    let hit = median(&mut || {
        std::hint::black_box(s.compute_pseudo(a, &pe, &host, &cx, &mut caches));
    });
    let other = median(&mut || {
        std::hint::black_box(s.compute_pseudo(b, &pe, &host, &cx, &mut caches));
    });
    let match_hit = median(&mut || {
        std::hint::black_box(s.matched_pseudo_rules(a, &pe, &mut caches));
    });
    let match_other = median(&mut || {
        std::hint::black_box(s.matched_pseudo_rules(b, &pe, &mut caches));
    });
    let has_hit = median(&mut || {
        std::hint::black_box(s.has_pseudo_rules(&pe));
    });
    let has_miss = median(&mut || {
        std::hint::black_box(s.has_pseudo_rules(&miss));
    });
    println!("part index bench: {rules} part rules");
    println!("  compute_pseudo, matching host (.host3 slider, part p7): {hit:.2} µs");
    println!("  compute_pseudo, other host (box.other, part p7):       {other:.2} µs");
    println!("  matched_pseudo_rules, matching host:                   {match_hit:.2} µs");
    println!("  matched_pseudo_rules, other host:                      {match_other:.2} µs");
    println!("  has_pseudo_rules, a used part:                        {has_hit:.3} µs");
    println!("  has_pseudo_rules, an unused part:                      {has_miss:.3} µs");
}

#[test]
fn part_rules_match_through_the_bucket() {
    let s = stylist(
        ".host1::part(thumb) { color: rgb(1, 0, 0) }
         .host2::part(thumb) { color: rgb(2, 0, 0) }
         .host1::part(track) { color: rgb(3, 0, 0) }
         slider::part(thumb):hover { opacity: 0.5 }
         #only::part(thumb) { letter-spacing: 2px }
         ::part(thumb) { font-weight: 700 }
         :hover::part(label) { opacity: 0.25 }
         calendar::part(day today) { color: rgb(4, 0, 0) }
         text-edit::placeholder { color: rgb(5, 0, 0) }
         .host2::selection { color: rgb(6, 0, 0) }
         slider { &::part(thumb):first-child { border-top-width: 1px } }",
    );
    let tree = Tree::parse("window\n  slider#only.host1 :hovered\n  box#b.host2\n  calendar#c\n  text-edit#t");
    let (a, b, c, t) = (tree.by_id("only"), tree.by_id("b"), tree.by_id("c"), tree.by_id("t"));
    let rule_text = |ids: Vec<RuleId>| -> Vec<String> { ids.into_iter().map(|r| cssparser::ToCss::to_css_string(&s.rule(r).selectors)).collect() };
    // only the thumb rules whose host compound names the element's class, id or tag, and the
    // host-free ones: never `.host2::part(thumb)`, `.host1::part(track)` or the label rule
    let tried = rule_text(s.pseudo_candidates(a, &PseudoElement::part("thumb")));
    // (nesting's `&` is `:is(slider)`, bucketed under `slider`)
    assert_eq!(tried, [".host1::part(thumb)", "slider::part(thumb):hover", "#only::part(thumb)", "::part(thumb)", ":is(slider)::part(thumb):first-child"], "{tried:?}");
    let tried = rule_text(s.pseudo_candidates(b, &PseudoElement::part("thumb")));
    assert_eq!(tried, [".host2::part(thumb)", "::part(thumb)"], "{tried:?}");
    assert!(s.pseudo_candidates(b, &PseudoElement::part("nope")).is_empty());
    // a multi-name part (P10) finds the rule through any of its names, once
    let tried = rule_text(s.pseudo_candidates(c, &PseudoElement::parts(&["day", "today", "weekend"])));
    assert_eq!(tried, ["calendar::part(day today)"]);
    assert_eq!(s.pseudo_candidates(c, &PseudoElement::part("day")).len(), 1, "tried, then rejected by the name test");
    assert!(s.pseudo_candidates(c, &PseudoElement::part("today")).is_empty(), "bucketed under its first name");
    assert_eq!(rule_text(s.pseudo_candidates(t, &PseudoElement::Placeholder)), ["text-edit::placeholder"]);
    assert!(s.pseudo_candidates(t, &PseudoElement::Selection).is_empty());
    assert_eq!(rule_text(s.pseudo_candidates(b, &PseudoElement::Selection)), [".host2::selection"]);

    // the bucketed candidates compute what the rules say
    let cx = ComputeContext::default();
    let mut caches = MatchCaches::new();
    let host = s.compute(a, None, None, &cx, &mut caches);
    let thumb = s.compute_pseudo_at(a, &PseudoElement::part("thumb"), Some((0, 2)), &host, &cx, &mut caches);
    assert_eq!(thumb.get("color"), Some(&ComputedValue::Color(Color { r: 1.0 / 255.0, g: 0.0, b: 0.0, a: 1.0 })));
    assert_eq!(thumb.get("opacity"), Some(&ComputedValue::Number(0.5)));
    assert_eq!(thumb.get("letter-spacing"), Some(&ComputedValue::Length(Length::px(2.0))));
    assert_eq!(thumb.get("font-weight"), Some(&ComputedValue::FontWeight(700)));
    assert_eq!(thumb.get("border-top-width"), Some(&ComputedValue::Length(Length::px(1.0))), "nested `&::part()` with P1");
    let label = s.compute_pseudo(a, &PseudoElement::part("label"), &host, &cx, &mut caches);
    assert_eq!(label.get("opacity"), Some(&ComputedValue::Number(0.25)));
    let today = s.compute_pseudo(c, &PseudoElement::parts(&["today", "day"]), &host, &cx, &mut caches);
    assert_eq!(today.get("color"), Some(&ComputedValue::Color(Color { r: 4.0 / 255.0, g: 0.0, b: 0.0, a: 1.0 })));
    assert_eq!(s.compute_pseudo(c, &PseudoElement::part("day"), &host, &cx, &mut caches).get("color"), host.get("color"));

    // has_pseudo_rules is a map lookup with the P10 name test
    assert!(s.has_pseudo_rules(&PseudoElement::part("thumb")));
    assert!(s.has_pseudo_rules(&PseudoElement::parts(&["day", "today"])));
    assert!(!s.has_pseudo_rules(&PseudoElement::part("today")));
    assert!(!s.has_pseudo_rules(&PseudoElement::part("day")));
    assert!(!s.has_pseudo_rules(&PseudoElement::part("nope")));
    assert!(s.has_pseudo_rules(&PseudoElement::Placeholder) && s.has_pseudo_rules(&PseudoElement::Selection));
    // pseudo-element rules are never candidates for the host itself
    assert!(s.matched_rules(a, &mut caches).is_empty());
}

#[test]
fn a_big_sheet_tries_one_bucket() {
    let s = stylist(&big_sheet());
    let tree = Tree::parse("window\n  slider#a.host3\n  box#b.other");
    // .host3::part(p7), slider::part(p7):hover, ::part(p7): 3 of 331
    assert_eq!(s.pseudo_candidates(tree.by_id("a"), &PseudoElement::part("p7")).len(), 3);
    assert_eq!(s.pseudo_candidates(tree.by_id("b"), &PseudoElement::part("p7")).len(), 1);
    assert!(s.pseudo_candidates(tree.by_id("b"), &PseudoElement::part("p8")).is_empty());
}
