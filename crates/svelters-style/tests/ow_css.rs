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
    // P1: state and logical structural pseudo-classes may follow `::part()` (not `of S`)
    for ok in [
        "button::part(label)",
        "slider:hover::part(thumb)",
        "text-edit::placeholder",
        "::selection",
        ".x::PART(track)",
        "a::part(x):hover",
        "tabs::part(tab):first-child",
        "tabs::part(tab):nth-child(2n+1):not(:hover)",
        "a::part(x):nth-last-child(odd), b:nth-child(2)",
    ] {
        parse_selector_list(ok, None).unwrap_or_else(|e| panic!("{ok}: {e}"));
    }
    for bad in ["a::before", "a::part()", "a::part(x)::placeholder", "a::part(x) b", "a::part(x):nth-child(2 of .y)", "a::placeholder:first-child", "a::part(x).y"] {
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
    // a later shorthand overrides an earlier longhand, and still sets the uniform property
    let cancel = style_of(&s, &tree, "cancel");
    assert_eq!(StyleView::new(&cancel).px("border-width"), Some(1.0));
    assert_eq!(StyleView::new(&cancel).color("border-color"), Some(rgb(0, 128, 0)));
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

#[test]
fn transition_longhands_cascade_with_the_shorthand() {
    // U2 E5: the four longhands, comma lists, and the shorthand expanding into them
    let s = stylist(
        "#ok { transition: background-color 1s linear 50ms; transition-duration: 150ms }
         #cancel { transition-duration: 150ms; transition: opacity 1s }
         #name { transition-property: background-color, opacity, color; transition-duration: 1s, 2s;
                 transition-timing-function: linear; transition-delay: 0s, 1s, 2s, 3s }
         #opt { transition-duration: 200ms }
         #tri { transition: color 1s, opacity 2s; transition-property: none }
         .colors { transition-property: color, background-color; transition-timing-function: ease-out; transition-duration: 150ms }
         .slow { transition-duration: 300ms }
         #row { --d: 75ms; transition: var(--d) border-top-color; transition-delay: var(--d) }",
    );
    let tree = Tree::parse(&format!("{TREE}    box#util.colors.slow\n    box#util2.slow.colors\n"));
    let list = |id: &str| StyleView::new(&style_of(&s, &tree, id)).transitions().to_vec();
    let t = |p: &str, d: f32, e: TimingFunction, l: f32| Transition { property: p.into(), duration: d, timing: e, delay: l };
    let (lin, ease, out) = (TimingFunction::LINEAR, TimingFunction::EASE, TimingFunction::CubicBezier(0.0, 0.0, 0.58, 1.0));
    // a later longhand changes only its part of the shorthand
    assert_eq!(list("ok"), [t("background-color", 0.15, lin, 0.05)]);
    // a later shorthand resets every longhand
    assert_eq!(list("cancel"), [t("opacity", 1.0, ease, 0.0)]);
    // `transition-property` sets the count; the other lists repeat or are cut
    assert_eq!(list("name"), [t("background-color", 1.0, lin, 0.0), t("opacity", 2.0, lin, 1.0), t("color", 1.0, lin, 2.0)]);
    // unset longhands take their initial values (`all 0s ease 0s`)
    assert_eq!(list("opt"), [t("all", 0.2, ease, 0.0)]);
    assert!(list("tri").is_empty(), "transition-property: none");
    // utility-style composition: the later rule's duration wins whatever the class order
    assert_eq!(list("util"), [t("color", 0.3, out, 0.0), t("background-color", 0.3, out, 0.0)]);
    assert_eq!(list("util2"), list("util"));
    // var() in the shorthand and a longhand
    assert_eq!(list("row"), [t("border-top-color", 0.075, ease, 0.075)]);
    // the longhands' own computed values
    let name = style_of(&s, &tree, "name");
    assert_eq!(name.get("transition-delay").map(|v| v.to_string()).as_deref(), Some("0s, 1s, 2s, 3s"));
    assert_eq!(name.get("transition-property").map(|v| v.to_string()).as_deref(), Some("background-color, opacity, color"));
    // nothing set: no list at all
    assert_eq!(style_of(&s, &tree, "form").get("transition"), None);

    // bad longhand values are errors
    let mut st = Stylist::new(&tables::EGUI);
    let sheet = Stylesheet::parse(
        "a { transition-duration: -1s } b { transition-property: none, color } c { transition-timing-function: bogus }
         d { transition-delay: 1px } e { transition-delay: -1s } f { transition: opacity 1s, none }",
        &ParseOptions::default(),
    );
    let (_, diags) = st.add_sheet(&sheet, Origin::Author);
    let errors: Vec<&str> = diags.iter().filter(|d| d.severity == Severity::Error).map(|d| d.message.as_str()).collect();
    assert_eq!(errors.len(), 5, "a negative delay is fine: {errors:?}");

    // the store runs on the assembled list, and changes to the longhands themselves don't transition
    let s = stylist(
        "button { background-color: rgb(0, 0, 0); transition-property: background-color; transition-duration: 1s; transition-timing-function: linear }
         button:hover { background-color: rgb(200, 100, 0); transition-duration: 2s }",
    );
    let idle = style_of(&s, &Tree::parse("window\n  button#b"), "b");
    let hover = style_of(&s, &Tree::parse("window\n  button#b :hovered"), "b");
    let mut tr: Transitions<u32> = Transitions::new();
    assert!(tr.restyled(1, &idle, &hover, 0.0, false));
    let c = tr.view(1, &hover, 1.0).background().unwrap();
    assert!((c.r - 100.0 / 255.0).abs() < 1e-3, "2 s from the hover rule: {c:?}");
    assert!(tr.sample(1, 1.0).unwrap().iter().all(|(i, _)| !tables::EGUI.props[*i].name.starts_with("transition")));
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

#[test]
fn logical_positions_after_parts() {
    // WIDGET-PARITY P1: repeated parts match their logical (model) position, not a sibling walk
    let s = stylist(
        "tabs::part(tab) { color: rgb(1, 1, 1) }
         tabs::part(tab):first-child { border-top-left-radius: 4px }
         tabs::part(tab):last-child { border-top-right-radius: 4px }
         tabs::part(tab):only-child { letter-spacing: 3px }
         tabs::part(tab):nth-child(even) { background-color: rgb(0, 0, 255) }
         tabs::part(tab):nth-last-child(2) { opacity: 0.5 }
         tabs::part(tab):hover { color: rgb(9, 9, 9) }
         tabs { &::part(tab):nth-child(odd) { font-weight: 700 } }",
    );
    let tree = Tree::parse("window\n  tabs#t\n  tabs#h :hovered");
    let cx = ComputeContext::default();
    let at = |id: &str, pos: Option<(usize, usize)>| {
        let host = style_of(&s, &tree, id);
        s.compute_pseudo_at(tree.by_id(id), &PseudoElement::part("tab"), pos, &host, &cx, &mut MatchCaches::new())
    };
    let px = |c: &ComputedStyle, n: &str| match c.get(n) {
        Some(ComputedValue::Length(l)) => Some(l.px),
        _ => None,
    };
    let blue = Some(&ComputedValue::Color(rgb(0, 0, 255)));
    let first = at("t", Some((0, 3)));
    assert_eq!(px(&first, "border-top-left-radius"), Some(4.0));
    assert_eq!(px(&first, "border-top-right-radius"), None);
    assert_ne!(first.get("background-color"), blue);
    assert_eq!(first.get("font-weight"), Some(&ComputedValue::FontWeight(700)), "nested `&::part():nth-child()`");
    let middle = at("t", Some((1, 3)));
    assert_eq!(middle.get("background-color"), blue);
    assert_eq!(middle.get("opacity"), Some(&ComputedValue::Number(0.5)));
    assert_eq!(px(&middle, "border-top-left-radius"), None);
    let last = at("t", Some((2, 3)));
    assert_eq!(px(&last, "border-top-right-radius"), Some(4.0));
    assert_eq!(px(&last, "letter-spacing"), None);
    let only = at("t", Some((0, 1)));
    assert_eq!((px(&only, "letter-spacing"), px(&only, "border-top-left-radius"), px(&only, "border-top-right-radius")), (Some(3.0), Some(4.0), Some(4.0)));
    // no logical position: only the position-free rules
    let none = at("t", None);
    assert_eq!((px(&none, "border-top-left-radius"), none.get("color")), (None, Some(&ComputedValue::Color(rgb(1, 1, 1)))));
    // a state pseudo-class after the part reads the host's state
    assert_eq!(at("h", None).get("color"), Some(&ComputedValue::Color(rgb(9, 9, 9))));
    assert!(s.invalidation().state_changed(StateMask::NONE, StateMask::HOVERED).contains(Reach::SELF));
    // serialized with the standard names
    let list = parse_selector_list("tabs::part(tab):nth-last-child(2n+1)", None).unwrap();
    assert_eq!(cssparser::ToCss::to_css_string(&list.slice()[0]), "tabs::part(tab):nth-last-child(2n+1)");
    // the position test itself
    assert!(Logical::Nth { a: 2, b: 0, from_end: false }.matches(Some((3, 10))));
    assert!(!Logical::Nth { a: 2, b: 0, from_end: false }.matches(Some((2, 10))));
    assert!(Logical::Nth { a: -1, b: 3, from_end: false }.matches(Some((2, 10))));
    assert!(!Logical::Nth { a: -1, b: 3, from_end: false }.matches(Some((3, 10))));
    assert!(!Logical::Only.matches(Some((0, 2))));
}

#[test]
fn radial_gradients() {
    let s = stylist(
        "#ok { background-image: radial-gradient(circle closest-side at 25% 50%, red, blue) }
         #cancel { background-image: radial-gradient(red, lime 40%, blue) }
         #name { background-image: linear-gradient(to right, red, blue) }",
    );
    let tree = Tree::parse(TREE);
    let ok = style_of(&s, &tree, "ok");
    let v = StyleView::new(&ok);
    assert_eq!(v.background_image(), None, "linear only");
    let Some(Gradient::Radial(g)) = v.gradient() else { panic!("{:?}", v.gradient()) };
    assert_eq!((g.circle, g.extent, g.center), (true, RadialExtent::ClosestSide, [0.25, 0.5]));
    let gr = v.gradient().unwrap();
    // closest side of (25, 25) in a 100x50 box: 25
    assert_eq!(gr.t_at([100.0, 50.0], [25.0, 25.0]), 0.0);
    assert!((gr.t_at([100.0, 50.0], [50.0, 25.0]) - 1.0).abs() < 1e-5);
    let mid = gr.color_at(0.5);
    assert!((mid.r - 0.5).abs() < 1e-3 && (mid.b - 0.5).abs() < 1e-3, "{mid:?}");
    assert!(ok.get("background-image").unwrap().to_string().starts_with("radial-gradient(circle closest-side at 25% 50%"));
    // the default: an ellipse through the farthest corner
    let cancel = style_of(&s, &tree, "cancel");
    let gr = StyleView::new(&cancel).gradient().unwrap().clone();
    assert_eq!(gr.stops().iter().map(|s| s.position).collect::<Vec<_>>(), [0.0, 0.4, 1.0]);
    assert!((gr.t_at([100.0, 50.0], [0.0, 0.0]) - 1.0).abs() < 1e-5);
    assert_eq!(gr.color_at(0.4), rgb(0, 255, 0));
    // linear: corners at 0 and 1 along the line
    let name = style_of(&s, &tree, "name");
    let gr = StyleView::new(&name).gradient().unwrap().clone();
    assert!(gr.t_at([100.0, 50.0], [0.0, 10.0]).abs() < 1e-5 && (gr.t_at([100.0, 50.0], [100.0, 40.0]) - 1.0).abs() < 1e-5);
    // explicit sizes and unknown positions are diagnosed
    for bad in ["radial-gradient(circle 10px, red, blue)", "radial-gradient(at middle, red, blue)", "radial-gradient(circle, red)"] {
        let sheet = Stylesheet::parse(&format!("a {{ background-image: {bad} }}"), &ParseOptions::default());
        let mut st = Stylist::new(&tables::EGUI);
        let (_, diags) = st.add_sheet(&sheet, Origin::Author);
        assert!(!sheet.diagnostics.is_empty() || !diags.is_empty(), "{bad} accepted");
    }
}

#[test]
fn radial_gradient_transitions_and_borrowed_overlays() {
    let s = stylist(
        "button { background-image: radial-gradient(circle at 0% 0%, red, blue); transition: background-image 1s linear }
         button:hover { background-image: radial-gradient(circle at 100% 100%, red, blue) }",
    );
    let idle = style_of(&s, &Tree::parse("window\n  button#b"), "b");
    let hover = style_of(&s, &Tree::parse("window\n  button#b :hovered"), "b");
    let mut tr: Transitions<u32> = Transitions::new();
    assert!(tr.restyled(1, &idle, &hover, 0.0, false));
    let over = tr.sample(1, 0.5).unwrap();
    // a borrowed overlay reads as the owned view
    let v = StyleView::with_overlay(&hover, &over);
    let Some(Gradient::Radial(g)) = v.gradient() else { panic!() };
    assert!((g.center[0] - 0.5).abs() < 1e-4 && (g.center[1] - 0.5).abs() < 1e-4, "{:?}", g.center);
    assert_eq!(v.gradient(), tr.view(1, &hover, 0.5).gradient());
    assert_eq!(overlay_get(&hover, &over, "background-image"), v.get("background-image"));
    assert_eq!(overlay_get(&hover, &over, "color"), hover.get("color"));
    assert!(tr.sample(2, 0.5).is_none());
}
