//! POL2-REGISTER: the open states a story shows without input, on the repository's own gallery
//! (`gallery/`): a widget's `tooltip` in the `hover` state (the pointer rests on the subject past
//! the platform's delay), `context-menu`'s `open` and `toast-host`'s `show`. GPU tests run only with
//! `SKELTER_GPU_TESTS=1`.

use skelter_gallery::{Cell, Extensions, Renderer, State};
use std::path::Path;

fn gpu() -> bool {
    let on = std::env::var("SKELTER_GPU_TESTS").as_deref() == Ok("1");
    if !on {
        eprintln!("skipped: set SKELTER_GPU_TESTS=1 to run GPU tests");
    }
    on
}

fn renderer() -> Renderer {
    Renderer::new(&Extensions::new().story_root(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gallery"))).unwrap()
}

fn cell(r: &Renderer, story: &str, variant: &str) -> Cell {
    let s = r.catalog().get(story).unwrap();
    Cell::new(s, s.variant(variant).unwrap())
}

#[test]
fn hover_shows_a_widgets_tooltip_after_the_delay() {
    if !gpu() {
        return;
    }
    let mut r = renderer();
    let mut c = cell(&r, "tooltip", "button");
    let m = r.mount(&c).unwrap();
    let window = r.harness().window();
    assert!(r.backend().with_doc(|d| d.window_tip_layer(window).is_none()), "no tip while not hovered");
    assert!(m.warnings.is_empty(), "{:?}", m.warnings);
    c.state = State::Hover;
    let m = r.mount(&c).unwrap();
    assert!(m.applied.as_deref().is_some_and(|a| a.contains("pointer over it")), "{:?}", m.applied);
    assert!(r.backend().with_doc(|d| d.window_tip_layer(window).is_some()), "the tip shows");
    // the next cell's pointer is gone: no tip, no hover
    c.state = State::Normal;
    r.mount(&c).unwrap();
    assert!(r.backend().with_doc(|d| d.window_tip_layer(window).is_none()), "the pointer left with the hover cell");
}

#[test]
fn a_context_menu_opens_and_toasts_show_without_input() {
    if !gpu() {
        return;
    }
    let mut r = renderer();
    let m = r.mount(&cell(&r, "context-menu", "open")).unwrap();
    assert!(m.warnings.is_empty(), "{:?}", m.warnings);
    let open = skelter_core::PropId::of("context-menu", "open", "bool");
    let opened = r.backend().with_doc(|d| d.query().kind(skelter_core::KindId::of("context-menu")).collect().into_iter().all(|n| d.get_prop(n, open) == Some(skelter_core::Value::Bool(true))));
    assert!(opened, "the menu is open");
    let m = r.mount(&cell(&r, "toast", "levels")).unwrap();
    assert!(m.warnings.is_empty(), "{:?}", m.warnings);
    let shown = r.backend().with_doc(|d| {
        let n = d.query().kind(skelter_core::KindId::of("toast-host")).collect()[0];
        d.with_kind(n, |k| k.as_any().and_then(|a| a.downcast_ref::<skelter_widgets::views::toast::ToastHost>()).unwrap().toasts().iter().filter(|t| t.shown()).count()).unwrap()
    });
    assert_eq!(shown, 4, "the four levels show");
}
