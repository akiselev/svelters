//! rmegui S step 3: the designer's reads and forced states on skelter, in svelters-egui's shapes
//! (`svelters_skelter::designer`): `scene().read` with `by_source`, `hit_stack` and matched rules
//! (with their `<style>` sources in dev builds), `pseudo_state` and `force_state`.

mod common;

use common::Rig;
use skelter_base::geom::{Point, Scale, Size};
use skelter_base::ids::WindowId;
use skelter_base::time::Time;
use svelters_abi::*;
use svelters_skelter::designer::{SourceLoc, StateBits};

const FILE: &str = "/ui/Card.svelte";
/// The `<style>` content starts at this offset of the sheet text (the compiler's map: one run).
const STYLE_AT: u32 = 200;

struct Ui {
    r: Rig,
    win: WindowId,
    cards: [svl_handle; 2],
    other: svl_handle,
}

fn ui() -> Ui {
    let r = Rig::new();
    let css = ".card { width: 40px; height: 20px }\n.card:hover { width: 60px }";
    let map = format!("0,{STYLE_AT},{}", css.len());
    let sheet = format!("Card.svelte{SVL_STYLE_SEP}{css}{SVL_STYLE_SEP}{FILE}{SVL_STYLE_SEP}{map}");
    r.br().style(SVL_STYLE_SHEET, 0, Some(&sheet)).unwrap();
    let root = r.create("box");
    let cards = [r.create("box"), r.create("box")];
    let other = r.create("box");
    for &c in &cards {
        r.br().style(SVL_STYLE_CLASSES, c, Some("card")).unwrap();
        // two rows of one `{#each}`: one element, rendered inside `<Card>` at Main.svelte#7
        r.set(c, "source-loc", Value::Str(format!("{FILE}#120\u{1f}/ui/Main.svelte#7")));
        r.insert(root, c);
    }
    r.set(other, "source-loc", Value::Str(format!("{FILE}#300")));
    r.insert(root, other);
    let win = WindowId(1);
    r.backend.attach_presented(root, win, Size::new(200.0, 100.0), Scale(1.0)).unwrap();
    frame(&r, win);
    Ui { r, win, cards, other }
}

fn frame(r: &Rig, win: WindowId) {
    r.backend.document().borrow_mut().frame(win, Time(0));
}

#[test]
fn by_source_finds_every_instance_of_an_element() {
    let u = ui();
    let (found, sites) = u.r.backend.scene().read(|s| {
        let found: Vec<svl_handle> = s.by_source(FILE, 120).map(|n| u64::from(n.id())).collect();
        let loc = s.get(found[0]).unwrap().source_loc().unwrap();
        let sites: Vec<(String, u32)> = loc.sites().map(|x| (x.file.to_string(), x.offset)).collect();
        (found, sites)
    });
    assert_eq!(found, u.cards);
    assert_eq!(sites, [("/ui/Main.svelte".to_string(), 7)]);
    assert_eq!(u.r.backend.scene().read(|s| s.by_source(FILE, 300).map(|n| n.id()).collect::<Vec<_>>()), [u.other]);
    let loc = SourceLoc::parse("/a/B.svelte#12\u{1f}/a/C.svelte#3").unwrap();
    assert_eq!((loc.element().file, loc.element().offset), ("/a/B.svelte", 12));
    assert!(SourceLoc::parse("no offset").is_none());
}

#[test]
fn matched_rules_carry_the_sheet_and_style_sources() {
    let u = ui();
    let rules = u.r.backend.scene().read(|s| s.get(u.cards[0]).unwrap().matched_rules());
    assert_eq!(rules.len(), 1, "{rules:?}");
    let rule = &rules[0];
    assert_eq!(rule.selectors, ".card");
    assert_eq!(rule.sheet.as_deref(), Some("Card.svelte"));
    let src = rule.source.as_ref().expect("a dev sheet has sources");
    assert_eq!((src.file.as_str(), src.style_offset), (FILE, STYLE_AT));
    let width = rule.decls.iter().find(|d| d.name == "width").unwrap();
    assert!(width.winner);
    assert_eq!(width.source.as_ref().unwrap().style_offset, STYLE_AT + ".card { ".len() as u32);
}

#[test]
fn a_forced_hover_matches_the_hover_rule_until_cleared() {
    let u = ui();
    let hover = svelters_skelter::style::pseudo_state(":hover").expect("hover is a state");
    assert_eq!(hover, StateBits::HOVERED);
    assert!(svelters_skelter::style::pseudo_state("enabled").is_none(), "negated");
    assert!(svelters_skelter::style::pseudo_state("bogus").is_none());
    let card = u.cards[0];
    let n = u.r.backend.node(card).unwrap();
    let width = || u.r.backend.document().borrow().world_rect(n).unwrap().width();
    assert_eq!(width(), 40.0);
    u.r.backend.force_state(card, StateBits(StateBits::NONE.0 | hover.0)).unwrap();
    assert_eq!(u.r.backend.forced_state(card), hover);
    frame(&u.r, u.win);
    assert_eq!(width(), 60.0);
    let rules = u.r.backend.scene().read(|s| s.get(card).unwrap().matched_rules());
    assert_eq!(rules.iter().map(|r| r.selectors.as_str()).collect::<Vec<_>>(), [".card", ".card:hover"]);
    // the node's own state is untouched: forcing is for matching only
    assert_eq!(u.r.backend.document().borrow().state(n), StateBits::NONE);
    u.r.backend.force_state(card, StateBits::NONE).unwrap();
    frame(&u.r, u.win);
    assert_eq!(width(), 40.0);
    assert!(u.r.backend.force_state(9_999_999, hover).is_err());
}

#[test]
fn hit_stack_lists_the_nodes_under_a_point_topmost_first() {
    let u = ui();
    let (first, root) = u.r.backend.scene().read(|s| {
        let c = s.get(u.cards[0]).unwrap();
        let r = c.drawn().unwrap();
        let p = Point::new(r.min.x + 1.0, r.min.y + 1.0);
        let stack: Vec<svl_handle> = s.hit_stack(u.win, p).into_iter().map(|t| t.node()).collect();
        (stack, c.parent().map(|p| p.id()))
    });
    assert_eq!(first.first(), Some(&u.cards[0]), "{first:?}");
    assert_eq!(first.get(1).copied(), root, "then the box it is in");
    assert!(u.r.backend.scene().read(|s| s.hit_stack(u.win, Point::new(-5.0, -5.0)).is_empty()));
}
