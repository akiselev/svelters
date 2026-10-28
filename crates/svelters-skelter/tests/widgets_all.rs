//! POL2-REGISTER: every skelter-widgets kind is mountable from Svelte (`surface::tier1`), with the
//! open states a story needs: `context-menu`'s `open`, `toast-host`'s `show`, `status-bar`'s
//! `show-message`.

mod common;

use common::Rig;
use svelters_abi::*;

#[test]
fn every_skelter_widgets_kind_has_a_hosted_surface_kind() {
    let mut reg = skelter_core::KindRegistry::new();
    skelter_widgets::host::register_tier0(&mut reg).unwrap();
    skelter_widgets::views::register_all(&mut reg).unwrap();
    skelter_widgets::table::kinds::register(&mut reg).unwrap();
    skelter_widgets::tabs::kinds::register(&mut reg).unwrap();
    skelter_widgets::pickers::kinds::register(&mut reg).unwrap();
    let kinds = svelters_skelter::kinds::Kinds::builtin().unwrap();
    let mut missing = Vec::new();
    for e in reg.iter() {
        let m = kinds.by_core(e.id).unwrap_or_else(|| panic!("{}: no surface kind", e.spec.name));
        assert!(m.hosted, "{}: a placeholder, not skelter-widgets' kind", e.spec.name);
        // every own core prop has a surface name that reaches it (the common ones through `Widget`)
        for p in e.spec.props {
            if !m.props.iter().any(|x| x.spec.name == p.name) {
                missing.push(format!("<{}> prop {}", m.tag(), p.name));
            }
        }
        for s in e.spec.signals {
            if !m.signal_named(s.name).is_some_and(|x| x.native.is_some()) {
                missing.push(format!("<{}> signal {}", m.tag(), s.name));
            }
        }
    }
    // members of the kinds registered before polish-b2 whose surface (egui's) has no name for them
    // (POL2-REGISTER Q1, closed by POL3-FINISH but one); a new gap fails here
    // `runs` stays: the surface has no list-of-records type, and Svelte gives a label its runs as
    // inline children (the `runs` slot), not as a prop
    const KNOWN: &[&str] = &["<label> prop runs"];
    missing.retain(|m| !KNOWN.contains(&m.as_str()));
    assert!(missing.is_empty(), "{missing:#?}");
}

#[test]
fn context_menu_open_is_a_native_prop() {
    let r = Rig::new();
    let c = r.create("context-menu");
    let n = r.backend.node(c).unwrap();
    let open = skelter_core::PropId::of("context-menu", "open", "bool");
    r.set(c, "open", Value::Bool(true));
    assert_eq!(r.backend.with_doc(|d| d.get_prop(n, open)), Some(skelter_core::Value::Bool(true)));
    assert_eq!(r.get(c, "open"), Value::Bool(true));
    r.call(c, "popdown");
    assert_eq!(r.get(c, "open"), Value::Bool(false), "the native state reads back");
}

#[test]
fn toast_host_shows_dismisses_and_clears_toasts() {
    use skelter_widgets::views::toast::ToastHost;
    let r = Rig::new();
    let t = r.create("toast-host");
    let n = r.backend.node(t).unwrap();
    let m = |name| r.br().resolve(NameKind::Method, r.kind(t), name);
    let first = r.br().call(t, m("show"), &[Value::Str("Saved".into())]).unwrap();
    assert_eq!(first, Value::I64(1), "show returns the toast's id");
    let second = r.br().call(t, m("show"), &[Value::Str("Overlap".into()), Value::Str("warning".into()), Value::F64(0.0), Value::List(vec![Value::Str("Fix".into())])]).unwrap();
    assert_eq!(second, Value::I64(2));
    let toasts = || r.backend.with_doc(|d| d.with_kind(n, |k| k.as_any().and_then(|a| a.downcast_ref::<ToastHost>()).unwrap().toasts().iter().map(|t| (t.id, t.text.clone(), t.level.name(), t.actions.clone())).collect::<Vec<_>>()).unwrap());
    assert_eq!(toasts(), [(1, "Saved".into(), "info", vec![]), (2, "Overlap".into(), "warning", vec!["Fix".to_string()])]);
    assert!(r.br().call(t, m("show"), &[Value::Str("x".into()), Value::Str("loud".into())]).is_err(), "an unknown level is refused");
    r.br().call(t, m("dismiss"), &[Value::I64(1)]).unwrap();
    assert_eq!(toasts().len(), 1);
    r.call(t, "clear");
    assert!(toasts().is_empty());
}

#[test]
fn status_bar_show_message_sets_the_message() {
    let r = Rig::new();
    let s = r.create("status-bar");
    let m = r.br().resolve(NameKind::Method, r.kind(s), "show-message");
    r.br().call(s, m, &[Value::Str("Routing…".into())]).unwrap();
    assert_eq!(r.get(s, "message"), Value::Str("Routing…".into()));
    r.call(s, "clear-message");
    assert_eq!(r.get(s, "message"), Value::Str(String::new()));
}

#[test]
fn modal_and_toast_host_resolve_their_members() {
    // their core ids hash into the host's app-export pack; the wire ids are moved out of it
    let r = Rig::new();
    for tag in ["modal", "toast-host"] {
        let h = r.create(tag);
        assert_ne!(r.kind(h) & svelters_skelter::kinds::APP_PACK, svelters_skelter::kinds::APP_PACK, "{tag}");
        assert_ne!(r.br().resolve(NameKind::Prop, r.kind(h), "anchor"), 0, "{tag}: Widget's anchor");
    }
}
