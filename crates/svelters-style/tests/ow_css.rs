//! OW-CSS (ARCH-WIDGETS §7.2): pseudo-elements, the state pseudo-classes of bits 14–22, outline,
//! per-side borders, cursor, text settings, gradients and the transitions store, on the egui table.

mod common;

use common::Tree;
use svelters_style::*;

const TREE: &str = "
window#root
  box#form
    button#ok.primary :focused :focus-visible :default
    button#cancel :focused
    checkbox#tri :indeterminate
    text-edit#name :placeholder-shown :required :user-invalid
    text-edit#opt
    box#row :current :drop-target :backdrop
";

fn ids(sel: &str) -> Vec<String> {
    let tree = Tree::parse(TREE);
    let list = parse_selector_list(sel, None).unwrap_or_else(|e| panic!("{sel}: {e}"));
    select(&list, tree.el(0), &mut MatchCaches::new()).iter().map(|e| e.tree.nodes[e.ix].id.clone().unwrap()).collect()
}

fn stylist(css: &str) -> Stylist {
    let mut s = Stylist::new(&tables::EGUI);
    let sheet = Stylesheet::parse(css, &ParseOptions::default());
    assert!(sheet.diagnostics.is_empty(), "{css}: {:?}", sheet.diagnostics);
    let (_, diags) = s.add_sheet(&sheet, Origin::Author);
    assert!(diags.is_empty(), "{css}: {diags:?}");
    s
}

fn style_of(s: &Stylist, tree: &Tree, id: &str) -> ComputedStyle {
    s.compute(tree.by_id(id), None, None, &ComputeContext::default(), &mut MatchCaches::new())
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: 1.0 }
}

#[test]
fn new_state_pseudo_classes() {
    let cases: &[(&str, &[&str])] = &[
        (":focus-visible", &["ok", "cancel"]), // default `is_focus_visible`: the bit, else focused
        (":indeterminate", &["tri"]),
        (":placeholder-shown", &["name"]),
        (":default", &["ok"]),
        (":drop-target", &["row"]),
        (":current", &["row"]),
        (":backdrop", &["row"]),
        (":required", &["name"]),
        ("text-edit:optional", &["opt"]),
        (":user-invalid", &["name"]),
    ];
    for (sel, want) in cases {
        assert_eq!(ids(sel), *want, "{sel}");
    }
    // bits 14–22 in the documented order
    let names: Vec<&str> = StateMask::BUILTIN[14..23].iter().map(|(n, _)| *n).collect();
    assert_eq!(names, ["focus-visible", "indeterminate", "placeholder-shown", "default", "drop-target", "current", "backdrop", "required", "user-invalid"]);
    for (i, (_, m)) in StateMask::BUILTIN.iter().enumerate() {
        assert_eq!(m.0, 1 << i);
    }
}

#[test]
fn pseudo_elements_parse_and_reject() {
    for ok in ["button::part(label)", "slider:hover::part(thumb)", "text-edit::placeholder", "::selection", ".x::PART(track)"] {
        parse_selector_list(ok, None).unwrap_or_else(|e| panic!("{ok}: {e}"));
    }
    for bad in ["a::before", "a::part(x):hover", "a::part()", "a::part(x)::placeholder", "a::part(x) b"] {
        assert!(parse_selector_list(bad, None).is_err(), "{bad} parsed");
    }
}

#[test]
fn pseudo_element_styles_compute_on_the_host() {
    let s = stylist(
        "button { color: red; background-color: blue }
         button::part(label) { background-color: green }
         button:focus::part(label) { letter-spacing: 2px }
         #cancel::part(label) { color: rgb(1, 2, 3) }
         text-edit::placeholder { color: gray }
         ::selection { background-color: rgb(0, 0, 255); color: white }",
    );
    let tree = Tree::parse(TREE);
    let cx = ComputeContext::default();
    let mut caches = MatchCaches::new();
    let ok = style_of(&s, &tree, "ok");
    // the pseudo-element's rule doesn't reach the host
    assert_eq!(ok.get("background-color"), Some(&ComputedValue::Color(rgb(0, 0, 255))));
    let label = s.compute_pseudo(tree.by_id("ok"), &PseudoElement::part("label"), &ok, &cx, &mut caches);
    assert_eq!(label.get("background-color"), Some(&ComputedValue::Color(rgb(0, 128, 0))));
    // inherits from the host (color), host state applies
    assert_eq!(label.get("color"), Some(&ComputedValue::Color(rgb(255, 0, 0))));
    assert_eq!(label.get("letter-spacing"), Some(&ComputedValue::Length(Length::px(2.0))));
    // the id rule beats the type rule
    let cancel = style_of(&s, &tree, "cancel");
    let label = s.compute_pseudo(tree.by_id("cancel"), &PseudoElement::part("label"), &cancel, &cx, &mut caches);
    assert_eq!(label.get("color"), Some(&ComputedValue::Color(rgb(1, 2, 3))));
    // other parts and pseudo-elements don't match
    let thumb = s.compute_pseudo(tree.by_id("ok"), &PseudoElement::part("thumb"), &ok, &cx, &mut caches);
    assert_eq!(thumb.get("background-color"), None);
    let name = style_of(&s, &tree, "name");
    let ph = s.compute_pseudo(tree.by_id("name"), &PseudoElement::Placeholder, &name, &cx, &mut caches);
    assert_eq!(ph.get("color"), Some(&ComputedValue::Color(rgb(128, 128, 128))));
    let sel = s.compute_pseudo(tree.by_id("name"), &PseudoElement::Selection, &name, &cx, &mut caches);
    assert_eq!(StyleView::new(&sel).background(), Some(rgb(0, 0, 255)));
    assert!(s.has_pseudo_rules(&PseudoElement::part("label")));
    assert!(!s.has_pseudo_rules(&PseudoElement::part("thumb")));
    // a state the part's rule reads invalidates the host (parts restyle with it)
    let reach = s.invalidation().state_changed(StateMask::NONE, StateMask::FOCUSED);
    assert!(reach.contains(Reach::SELF), "{reach:?}");
}

#[test]
fn borders_outline_and_radii() {
    let s = stylist(
        "#ok { border: 1px solid red; border-left: 3px dashed blue; border-radius: 4px 2px;
               outline: 2px solid rgb(0, 0, 255); outline-offset: -1px }
         #cancel { border-top-width: 5px; border-width: 1px; border-color: green }
         #opt { border-width: 2px; border-style: none solid; outline: auto; border-top-left-radius: 9px; border-radius: 1px }",
    );
    let tree = Tree::parse(TREE);
    let ok = style_of(&s, &tree, "ok");
    let v = StyleView::new(&ok);
    let b = v.border();
    assert_eq!(b[0], BorderSide { width: 1.0, style: "solid", color: Some(rgb(255, 0, 0)) });
    assert_eq!(b[3], BorderSide { width: 3.0, style: "dashed", color: Some(rgb(0, 0, 255)) });
    assert!(!v.border_is_uniform());
    assert_eq!(v.radii(), [4.0, 2.0, 4.0, 2.0]);
    assert_eq!(v.outline(), Outline { width: 2.0, style: "solid", color: Some(rgb(0, 0, 255)), offset: -1.0 });
    // the uniform props the CS5g paint reads are set by the shorthand too
    assert_eq!(v.px("border-width"), Some(1.0));
    // a later shorthand overrides an earlier longhand
    let cancel = style_of(&s, &tree, "cancel");
    let v = StyleView::new(&cancel);
    assert_eq!(v.border_side(Side::Top).width, 1.0);
    assert!(v.border_is_uniform());
    let opt = style_of(&s, &tree, "opt");
    let v = StyleView::new(&opt);
    assert_eq!(v.border_side(Side::Top).width, 0.0, "style none");
    assert_eq!(v.border_side(Side::Right).width, 2.0);
    assert_eq!(v.outline().style, "auto");
    assert_eq!(v.outline().width, 3.0, "medium");
    assert_eq!(v.radii(), [1.0; 4], "the later border-radius wins over the corner");
    // unset outline: none
    let name = style_of(&s, &tree, "name");
    assert_eq!(StyleView::new(&name).outline().width, 0.0);
}

#[test]
fn shorthand_and_value_diagnostics() {
    let mut s = Stylist::new(&tables::EGUI);
    let sheet = Stylesheet::parse(
        "a { outline: 1px 2px } b { border-style: wavy } c { border-top: 1px solid red }
         d { transition: color 1s bogus } e { background-image: linear-gradient(red) } f { cursor: hand }",
        &ParseOptions::default(),
    );
    let (_, diags) = s.add_sheet(&sheet, Origin::Author);
    let errors: Vec<&str> = diags.iter().filter(|d| d.severity == Severity::Error).map(|d| d.message.as_str()).collect();
    assert_eq!(errors.len(), 5, "{errors:?}");
    assert!(diags.iter().all(|d| !d.message.contains("not an egui style property")), "{diags:?}");
}

#[test]
fn text_cursor_and_inheritance() {
    let s = stylist(
        "#form { cursor: pointer; text-transform: uppercase; letter-spacing: normal; text-overflow: ellipsis }
         #ok { letter-spacing: 0.5em; font-size: 10px }",
    );
    let tree = Tree::parse(TREE);
    let cx = ComputeContext::default();
    let mut caches = MatchCaches::new();
    let form = s.compute(tree.by_id("form"), None, None, &cx, &mut caches);
    let ok = s.compute(tree.by_id("ok"), Some(&form), None, &cx, &mut caches);
    let v = StyleView::new(&ok);
    assert_eq!(v.cursor(), Some("pointer"), "cursor inherits");
    let t = v.text();
    assert_eq!(t.transform, "uppercase", "text-transform inherits");
    assert_eq!(t.letter_spacing, 5.0);
    assert_eq!(t.overflow, "clip", "text-overflow does not inherit");
    assert_eq!(t.transform("Save as…"), "SAVE AS…");
    assert_eq!(TextSettings { transform: "capitalize", letter_spacing: 0.0, overflow: "clip" }.transform("open file-a now"), "Open File-a Now");
    assert_eq!(StyleView::new(&form).text().letter_spacing, 0.0);
}

#[test]
fn gradients() {
    let s = stylist("#ok { background-image: linear-gradient(to right, red, blue 80%) } #cancel { background-image: linear-gradient(red, lime, blue) }");
    let tree = Tree::parse(TREE);
    let ok = style_of(&s, &tree, "ok");
    let g = StyleView::new(&ok).background_image().unwrap().clone();
    assert_eq!(g.angle, 90.0);
    assert_eq!(g.stops.iter().map(|s| s.position).collect::<Vec<_>>(), [0.0, 0.8]);
    let cancel = style_of(&s, &tree, "cancel");
    let g = StyleView::new(&cancel).background_image().unwrap().clone();
    assert_eq!(g.angle, 180.0);
    assert_eq!(g.stops.iter().map(|s| s.position).collect::<Vec<_>>(), [0.0, 0.5, 1.0]);
}

#[test]
fn transitions_parse() {
    let s = stylist("#ok { transition: background-color 200ms ease-in, opacity .5s linear 100ms } #cancel { transition: none }");
    let tree = Tree::parse(TREE);
    let ok = style_of(&s, &tree, "ok");
    let t = StyleView::new(&ok).transitions().to_vec();
    assert_eq!(t.len(), 2);
    assert_eq!((&*t[0].property, t[0].duration, t[0].delay), ("background-color", 0.2, 0.0));
    assert_eq!(t[0].timing, TimingFunction::CubicBezier(0.42, 0.0, 1.0, 1.0));
    assert_eq!((&*t[1].property, t[1].duration, t[1].delay, t[1].timing), ("opacity", 0.5, 0.1, TimingFunction::LINEAR));
    assert!(StyleView::new(&style_of(&s, &tree, "cancel")).transitions().is_empty());
    let e = TimingFunction::EASE;
    assert_eq!(e.apply(0.0), 0.0);
    assert!((e.apply(1.0) - 1.0).abs() < 1e-4);
    assert!((e.apply(0.5) - 0.8024).abs() < 1e-2, "{}", e.apply(0.5));
    assert_eq!(TimingFunction::Steps(4, false).apply(0.3), 0.25);
}

/// A button whose background transitions over 1 s, except while pressed (DESIGN R2).
const SHEET: &str = "
    button { background-color: rgb(0, 0, 0); opacity: 1; transition: background-color 1s linear, opacity 1s linear }
    button:hover { background-color: rgb(200, 100, 0); opacity: 0.5 }
    button:active { background-color: rgb(255, 255, 255); transition: none }
";

fn button(state: &str) -> ComputedStyle {
    let s = stylist(SHEET);
    let tree = Tree::parse(&format!("window\n  button#b {state}"));
    style_of(&s, &tree, "b")
}

#[test]
fn transitions_sample_retarget_and_end() {
    let (idle, hover) = (button(""), button(":hovered"));
    let mut tr: Transitions<u32> = Transitions::new();
    assert!(tr.restyled(1, &idle, &hover, 10.0, false));
    // halfway, linear
    let v = tr.view(1, &hover, 10.5);
    assert!(v.is_animated());
    let c = v.background().unwrap();
    assert!((c.r - 100.0 / 255.0).abs() < 1e-3 && (c.g - 50.0 / 255.0).abs() < 1e-3, "{c:?}");
    assert!((v.opacity() - 0.75).abs() < 1e-4);
    assert!(tr.tick(10.5));
    // un-hover at 10.5: back from the painted value, not from the hover colour
    assert!(tr.restyled(1, &hover, &idle, 10.5, false));
    let c = tr.view(1, &idle, 10.5).background().unwrap();
    assert!((c.r - 100.0 / 255.0).abs() < 1e-3, "{c:?}");
    let c = tr.view(1, &idle, 11.0).background().unwrap();
    assert!((c.r - 50.0 / 255.0).abs() < 1e-3, "{c:?}");
    // ends
    assert!(!tr.tick(11.6));
    assert!(!tr.is_running(1));
    assert_eq!(tr.view(1, &idle, 11.6).background(), Some(Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }));
}

#[test]
fn pressed_and_reduced_motion_are_instant() {
    let (hover, active) = (button(":hovered"), button(":hovered :pressed"));
    let mut tr: Transitions<u32> = Transitions::new();
    // `transition: none` on :active: no transition into the pressed colour
    assert!(!tr.restyled(1, &hover, &active, 0.0, false));
    assert!(!tr.view(1, &active, 0.1).is_animated());
    // reduced motion: nothing starts, running ones end
    let idle = button("");
    assert!(tr.restyled(2, &idle, &hover, 0.0, false));
    assert!(!tr.restyled(2, &hover, &idle, 0.2, true));
    assert!(!tr.is_running(2));
    assert!(!tr.restyled(3, &idle, &hover, 0.0, true));
    assert_eq!(tr.running_keys().count(), 0);
}

#[test]
fn zero_duration_with_delay_switches_after_the_delay() {
    // R11: the sash bar appears 300ms after hover; keywords switch too
    let s = stylist("sash { background-color: rgb(0, 0, 0); cursor: default; transition: all 0s 300ms } sash:hover { background-color: rgb(0, 0, 255); cursor: col-resize }");
    let idle = style_of(&s, &Tree::parse("window\n  sash#s"), "s");
    let hover = style_of(&s, &Tree::parse("window\n  sash#s :hovered"), "s");
    let mut tr: Transitions<u32> = Transitions::new();
    assert!(tr.restyled(1, &idle, &hover, 0.0, false));
    assert_eq!(tr.view(1, &hover, 0.2).background(), Some(rgb(0, 0, 0)));
    assert_eq!(tr.view(1, &hover, 0.2).cursor(), Some("default"));
    assert!(tr.tick(0.2));
    assert_eq!(tr.view(1, &hover, 0.31).background(), Some(rgb(0, 0, 255)));
    assert!(!tr.tick(0.31));
}

#[test]
fn text_primitive_properties() {
    let s = stylist("#form { font-weight: bold; font-style: italic; white-space: nowrap; text-align: center; font-family: \"Segoe UI\", sans-serif; line-height: 1.4 }
                     #ok:hover, #ok { text-decoration-line: underline }");
    let tree = Tree::parse(TREE);
    let cx = ComputeContext::default();
    let mut caches = MatchCaches::new();
    let form = s.compute(tree.by_id("form"), None, None, &cx, &mut caches);
    let ok = s.compute(tree.by_id("ok"), Some(&form), None, &cx, &mut caches);
    let v = StyleView::new(&ok);
    assert_eq!(v.get("font-weight"), Some(&ComputedValue::FontWeight(700)));
    assert_eq!(v.keyword("font-style"), Some("italic"));
    assert_eq!(v.keyword("white-space"), Some("nowrap"));
    assert_eq!(v.keyword("text-align"), Some("center"));
    assert_eq!(v.keyword("text-decoration-line"), Some("underline"));
    assert_eq!(v.get("line-height"), Some(&ComputedValue::Raw("1.4".into())));
    assert_eq!(StyleView::new(&form).keyword("text-decoration-line"), None, "not inherited");
}

#[test]
fn multi_name_parts_and_has_children() {
    // P10: a part with several names matches rules for any subset; P5: :has-children (bit 23)
    let s = stylist(
        "calendar::part(day) { color: rgb(1, 1, 1) }
         calendar::part(day today) { background-color: rgb(0, 0, 255) }
         calendar::part(day weekend) { color: rgb(255, 0, 0) }
         row:has-children { cursor: pointer }",
    );
    let tree = Tree::parse("window\n  calendar#c\n  row#r :has-children\n  row#leaf");
    let cx = ComputeContext::default();
    let mut caches = MatchCaches::new();
    let host = style_of(&s, &tree, "c");
    let today = s.compute_pseudo(tree.by_id("c"), &PseudoElement::parts(&["today", "day"]), &host, &cx, &mut caches);
    let v = StyleView::new(&today);
    assert_eq!((v.color("color"), v.background()), (Some(rgb(1, 1, 1)), Some(rgb(0, 0, 255))));
    let weekend = s.compute_pseudo(tree.by_id("c"), &PseudoElement::parts(&["day", "weekend"]), &host, &cx, &mut caches);
    let v = StyleView::new(&weekend);
    assert_eq!((v.color("color"), v.background()), (Some(rgb(255, 0, 0)), None));
    assert!(s.has_pseudo_rules(&PseudoElement::parts(&["day", "today", "weekend"])));
    assert!(!s.has_pseudo_rules(&PseudoElement::part("today")), "`day today` needs both names");
    assert_eq!(StyleView::new(&style_of(&s, &tree, "r")).cursor(), Some("pointer"));
    assert_eq!(StyleView::new(&style_of(&s, &tree, "leaf")).cursor(), None);
    assert_eq!(StateMask::BUILTIN[23], ("has-children", StateMask::HAS_CHILDREN));
    assert_eq!(StateMask::HAS_CHILDREN.0, 1 << 23);
}

#[test]
fn glyph_content_and_part_painters() {
    // P2 glyphs from CSS; WIDGET-PARITY Q1 painters selected with paint(name)
    let s = stylist(
        "checkbox:checked::part(mark) { content: icon(check) }
         checkbox:indeterminate::part(mark) { content: icon(dash) }
         combo-box::part(chevron) { content: url(\"asset://icons/chev.svg\") }
         slider::part(thumb) { background-image: paint(cad.knurl) }
         slider::part(track) { content: paint(cad.ticks) }
         button::part(mark) { content: none }",
    );
    let tree = Tree::parse("window\n  checkbox#a :checked\n  checkbox#b :indeterminate\n  combo-box#c\n  slider#s");
    let cx = ComputeContext::default();
    let mut caches = MatchCaches::new();
    let part = |id: &str, p: &str| {
        let host = style_of(&s, &tree, id);
        s.compute_pseudo(tree.by_id(id), &PseudoElement::part(p), &host, &cx, &mut MatchCaches::new())
    };
    let _ = &mut caches;
    assert_eq!(StyleView::new(&part("a", "mark")).content(), Some(&Content::Icon("check".into())));
    assert_eq!(StyleView::new(&part("b", "mark")).content(), Some(&Content::Icon("dash".into())));
    assert_eq!(StyleView::new(&part("c", "chevron")).content(), Some(&Content::Url("asset://icons/chev.svg".into())));
    let thumb = part("s", "thumb");
    assert_eq!(StyleView::new(&thumb).background_painter(), Some("cad.knurl"));
    assert_eq!(StyleView::new(&thumb).background_image(), None);
    assert_eq!(StyleView::new(&part("s", "track")).content(), Some(&Content::Paint("cad.ticks".into())));
    // bad values are errors
    let mut st = Stylist::new(&tables::EGUI);
    let sheet = Stylesheet::parse("a { content: icon() } b { background-image: paint(1x) } c { content: \"\" }", &ParseOptions::default());
    let (_, diags) = st.add_sheet(&sheet, Origin::Author);
    assert_eq!(diags.iter().filter(|d| d.severity == Severity::Error).count(), 3, "{diags:?}");
}
