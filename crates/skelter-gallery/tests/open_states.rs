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

/// R2 (the app runner fires the kinds' timers): a toast shown through `show` with a duration times
/// out on the virtual clock, with no gallery-side timer code.
#[test]
fn a_toast_times_out_under_the_app_runner() {
    if !gpu() {
        return;
    }
    let mut r = renderer();
    r.mount(&cell(&r, "toast", "levels")).unwrap();
    let toasts = |r: &Renderer| {
        r.backend().with_doc(|d| {
            let n = d.query().kind(skelter_core::KindId::of("toast-host")).collect()[0];
            d.with_kind(n, |k| k.as_any().and_then(|a| a.downcast_ref::<skelter_widgets::views::toast::ToastHost>()).unwrap().toasts().len()).unwrap()
        })
    };
    assert_eq!(toasts(&r), 4, "the story's four, until closed");
    let show = r.backend().with_doc(|d| {
        let n = d.query().kind(skelter_core::KindId::of("toast-host")).collect()[0];
        let m = d.registry().get(skelter_core::KindId::of("toast-host")).unwrap().spec.method_named("show").unwrap();
        d.call(n, m, &[skelter_core::Value::from("Saved"), skelter_core::Value::from("info"), skelter_core::Value::F64(0.5)]).map(|(ret, _)| ret)
    });
    assert!(matches!(show, Ok(Some(skelter_core::Value::I64(_)))), "{show:?}");
    assert_eq!(toasts(&r), 5);
    r.harness().run_for(skelter_base::time::Span::from_millis(800));
    assert_eq!(toasts(&r), 4, "the half-second toast timed out");
}
