//! A WPT-style selector corpus: each case is a selector and the ids it must select from one tree.

mod common;

use common::Tree;
use svelters_style::{MatchCaches, StateMask, parse_selector_list, select};

const TREE: &str = "
window#root
  box#main.panel.svelte-xyz[role=group]
    label#l1.title.svelte-xyz[text=Hello]
    button#b1.primary.svelte-xyz[kind=push-button] :hovered
    button#b2.svelte-xyz[kind=toggle] :focused :checked
    box#empty.svelte-xyz
    entry#e1.svelte-xyz[placeholder=Name][value=] :disabled
  box#side
    label#l2[lang=en-US]
    label#l3.title
    button#b3.danger :pressed
";

fn ids(sel: &str) -> Vec<String> {
    let tree = Tree::parse(TREE);
    let states = [("cad.pending", StateMask(1 << 32))];
    let list = parse_selector_list(sel, Some(&states)).unwrap_or_else(|e| panic!("{sel}: {e}"));
    select(&list, tree.el(0), &mut MatchCaches::new()).iter().map(|e| e.tree.nodes[e.ix].id.clone().unwrap()).collect()
}

#[test]
fn corpus() {
    let cases: &[(&str, &[&str])] = &[
        // type, class, id, universal
        ("button", &["b1", "b2", "b3"]),
        (".title", &["l1", "l3"]),
        ("#b2", &["b2"]),
        ("*", &["root", "main", "l1", "b1", "b2", "empty", "e1", "side", "l2", "l3", "b3"]),
        ("Button", &[]),
        // attributes
        ("[kind]", &["b1", "b2"]),
        ("[kind=toggle]", &["b2"]),
        ("[kind^=push]", &["b1"]),
        ("[kind$=button]", &["b1"]),
        ("[kind*=sh-bu]", &["b1"]),
        ("[text~=Hello]", &["l1"]),
        ("[lang|=en]", &["l2"]),
        ("[kind=TOGGLE i]", &["b2"]),
        ("[value='']", &["e1"]),
        // combinators
        ("box > label", &["l1", "l2", "l3"]),
        ("window label", &["l1", "l2", "l3"]),
        ("label + button", &["b1", "b3"]),
        ("label ~ button", &["b1", "b2", "b3"]),
        ("#main > :last-child", &["e1"]),
        // logical
        (":is(#b1, #l3)", &["b1", "l3"]),
        (":where(.danger, .primary)", &["b1", "b3"]),
        ("button:not(.primary)", &["b2", "b3"]),
        ("box:has(> .danger)", &["side"]),
        ("box:has(button:checked:hover)", &[]),
        ("box:has(button:hover)", &["main"]),
        ("label:has(+ button)", &["l1", "l3"]),
        ("window:has(.danger)", &["root"]),
        // structural
        (":root", &["root"]),
        (":empty", &["l1", "b1", "b2", "empty", "e1", "l2", "l3", "b3"]),
        ("#main > :first-child", &["l1"]),
        ("label:only-child", &[]),
        ("#main > :nth-child(2n)", &["b1", "empty"]),
        ("#main > :nth-last-child(1)", &["e1"]),
        ("#main > :nth-child(odd of button)", &["b1"]),
        ("button:nth-of-type(2)", &["b2"]),
        ("label:first-of-type", &["l1", "l2"]),
        // Svelte's scoped shape
        (".panel.svelte-xyz > button:where(.svelte-xyz):hover", &["b1"]),
        // states
        (":hover", &["b1"]),
        (":active", &["b3"]),
        (":focus", &["b2"]),
        (":focus-visible", &["b2"]),
        (":checked", &["b2"]),
        (":disabled", &["e1"]),
        ("entry:enabled", &[]),
        ("button:enabled", &["b1", "b2", "b3"]),
        (":state(cad.pending)", &[]),
    ];
    let mut failures = Vec::new();
    for (sel, want) in cases {
        let got = ids(sel);
        if got != *want {
            failures.push(format!("{sel}: got {got:?}, want {want:?}"));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn rejects() {
    for bad in ["a::before", ":nope", ":state(cad.missing)", "a >", "[x=", ":nth-child(x)", "a,,b"] {
        assert!(parse_selector_list(bad, None).is_err(), "{bad} parsed");
    }
}

#[test]
fn specificity() {
    let spec = |s: &str| {
        let l = parse_selector_list(s, None).unwrap();
        let v = l.slice()[0].specificity();
        (v >> 20, (v >> 10) & 0x3ff, v & 0x3ff)
    };
    assert_eq!(spec(".a.svelte-xyz > button:where(.svelte-xyz):hover"), (0, 3, 1));
    assert_eq!(spec("box:has(> button:hover)"), (0, 1, 2));
    assert_eq!(spec("label:nth-child(2):is(label):not(.q)"), (0, 2, 2));
    assert_eq!(spec("#a .b c"), (1, 1, 1));
    assert_eq!(spec("*"), (0, 0, 0));
}
