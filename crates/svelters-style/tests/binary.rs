//! Precompiled sheets (`binary`): a sheet written as bytes and read back styles every element
//! exactly as the sheet parsed from its text, and hints and utilities survive the round trip.

mod common;

use common::Tree;
use svelters_style::binary::{ElementHints, Precompiled, compound_matches, decode, encode};
use svelters_style::*;

const TREE: &str = "
window#root.app
  box#card.card.dark.svelte-x1
    label#title.title.svelte-x1
    button#ok.primary.svelte-x1 :hovered
    button#no.svelte-x1 :focused
  box#side.side
    label#hint.hint.muted
";

const SHEETS: &[&str] = &[
    // nesting, :is(), specificity, source order, !important
    ".card.svelte-x1 { padding: 4px; .title { color: red; } & > button { gap: 3px } } #ok.svelte-x1 { gap: 9px !important } button.svelte-x1 { gap: 2px }",
    // state pseudo-classes, :not, structural pseudo-classes
    "button:hover { color: #00ff00 } button:not(.primary) { color: blue } box > :first-child { font-size: 20px } label:last-child { font-size: 11px }",
    // @media and @layer (named and anonymous), var()
    "@layer base, components; @layer components { .title { color: rgb(1, 2, 3) } } @layer base { .title { color: rgb(9, 9, 9); padding: 1px } } @layer { .hint { gap: 7px } } @media (min-width: 600px) { .muted { color: gray } } @media (max-width: 10px) { .muted { color: red } }",
    ":root { --accent: #123456 } .side { --gap: 5px } .hint { color: var(--accent); gap: var(--gap) }",
    // identical blocks share one interned block
    ".a { gap: 1px } .b { gap: 1px } .title { gap: 1px }",
];

fn styles(sheet: &Stylesheet, tree: &Tree) -> Vec<ComputedStyle> {
    let mut stylist = Stylist::new(&tables::EGUI);
    let (_, diags) = stylist.add_sheet(sheet, Origin::Author);
    assert!(diags.is_empty(), "{diags:?}");
    let mut caches = MatchCaches::new();
    let mut out: Vec<ComputedStyle> = Vec::new();
    // parents come before children in the outline
    for el in tree.all() {
        let parent = tree.nodes[el.ix].parent.map(|p| out[p].clone());
        out.push(stylist.compute(el, parent.as_ref(), None, &ComputeContext::default(), &mut caches));
    }
    out
}

#[test]
fn precompiled_sheets_style_like_parsed_ones() {
    let tree = Tree::parse(TREE);
    for css in SHEETS {
        let parsed = Stylesheet::parse(css, &ParseOptions::default());
        assert!(parsed.diagnostics.is_empty(), "{css}: {:?}", parsed.diagnostics);
        let bytes = encode(&Precompiled { sheet: parsed.clone(), ..Precompiled::default() });
        let loaded = decode(&bytes, &ParseOptions::default()).unwrap_or_else(|e| panic!("{css}: {e}"));
        assert_eq!(loaded.sheet.rules.len(), parsed.rules.len(), "{css}");
        for (a, b) in loaded.sheet.rules.iter().zip(&parsed.rules) {
            assert_eq!(a.selector_text(), b.selector_text(), "{css}");
            // interned blocks keep the positions of their first rule
            let key = |d: &Declaration| (d.name.clone(), d.value.clone(), d.important, d.has_var);
            assert_eq!(a.block.decls.iter().map(key).collect::<Vec<_>>(), b.block.decls.iter().map(key).collect::<Vec<_>>(), "{css}");
            assert_eq!(a.media.iter().map(|m| m.text.clone()).collect::<Vec<_>>(), b.media.iter().map(|m| m.text.clone()).collect::<Vec<_>>(), "{css}");
            assert_eq!((a.line, a.column), (b.line, b.column), "{css}");
        }
        assert_eq!(loaded.sheet.layers.len(), parsed.layers.len(), "{css}");
        let (want, got) = (styles(&parsed, &tree), styles(&loaded.sheet, &tree));
        for (i, (w, g)) in want.iter().zip(&got).enumerate() {
            assert_eq!(g, w, "{css}: element {i} ({:?})", tree.nodes[i].tag);
        }
    }
}

#[test]
fn identical_blocks_are_stored_once() {
    let parsed = Stylesheet::parse(SHEETS[4], &ParseOptions::default());
    let one = encode(&Precompiled { sheet: Stylesheet { rules: parsed.rules[..1].to_vec(), ..parsed.clone() }, ..Precompiled::default() });
    let three = encode(&Precompiled { sheet: parsed.clone(), ..Precompiled::default() });
    // two more rules cost their selectors and positions, not their declarations
    assert!(three.len() - one.len() < 2 * 16, "{} vs {}", three.len(), one.len());
    let loaded = decode(&three, &ParseOptions::default()).unwrap();
    assert!(std::sync::Arc::ptr_eq(&loaded.sheet.rules[0].block, &loaded.sheet.rules[2].block));
}

#[test]
fn anonymous_layers_stay_distinct_across_loads() {
    let parsed = Stylesheet::parse("@layer { .a { gap: 1px } }", &ParseOptions::default());
    let bytes = encode(&Precompiled { sheet: parsed, ..Precompiled::default() });
    let a = decode(&bytes, &ParseOptions::default()).unwrap();
    let b = decode(&bytes, &ParseOptions::default()).unwrap();
    assert_ne!(a.sheet.rules[0].layer, b.sheet.rules[0].layer);
}

#[test]
fn hints_globals_and_utilities_round_trip() {
    let tokens = UtilityTokens::builtin();
    let res = Resolver::new(&tables::EGUI, &tokens);
    let Resolved::Utility(u) = res.resolve("p-2") else { panic!("p-2 is a utility") };
    let p = Precompiled {
        sheet: Stylesheet::parse(".x { gap: 1px } .y { gap: 2px }", &ParseOptions::default()),
        hints: vec![ElementHints { element: 120, candidates: vec![0, 1], always: vec![0] }, ElementHints { element: 300, candidates: vec![], always: vec![] }],
        global_rules: vec![1],
        utilities: vec![u.clone()],
    };
    let back = decode(&encode(&p), &ParseOptions::default()).unwrap();
    assert_eq!(back.hints, p.hints);
    assert_eq!(back.global_rules, p.global_rules);
    assert_eq!(back.utilities, vec![u.clone()]);

    // a stylist preloads them without resolving
    let mut s = Stylist::new(&tables::EGUI);
    s.set_utilities(UtilityConfig::enabled());
    assert_eq!(s.preload_utilities(&back.utilities, &ParseOptions::default()), 1);
    assert!(s.is_utility("p-2"));
    assert_eq!(s.preload_utilities(&back.utilities, &ParseOptions::default()), 0, "already known");
}

#[test]
fn decode_refuses_garbage() {
    assert!(decode(b"nope", &ParseOptions::default()).is_err());
    let mut bytes = encode(&Precompiled { sheet: Stylesheet::parse(".a { gap: 1px }", &ParseOptions::default()), ..Precompiled::default() });
    bytes.pop();
    assert!(decode(&bytes, &ParseOptions::default()).is_err());
}

#[test]
fn compound_selectors_match_static_attributes() {
    assert_eq!(compound_matches("label.title.svelte-x1", "label", &["title", "svelte-x1"], None), Some(true));
    assert_eq!(compound_matches(".title", "label", &["other"], None), Some(false));
    assert_eq!(compound_matches("#ok", "button", &[], Some("ok")), Some(true));
    assert_eq!(compound_matches("button", "label", &[], None), Some(false));
    assert_eq!(compound_matches("*", "label", &[], None), Some(true));
    // context or state: decided at run time
    assert_eq!(compound_matches(".card .title", "label", &["title"], None), None);
    assert_eq!(compound_matches("button:hover", "button", &[], None), None);
    assert_eq!(compound_matches("box > label", "label", &[], None), None);
    assert_eq!(compound_matches("[data-x]", "label", &[], None), None);
}
