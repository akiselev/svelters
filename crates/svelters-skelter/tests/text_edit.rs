//! qc-a11y §3.2: a Svelte `<text-edit>` is skelter-widgets' text field, so it carries the field's
//! accessibility text (`Role::TextRun` children, the selection) that AT-SPI's Text interface reads.

mod common;

use common::Rig;
use svelters_abi::*;

#[test]
fn text_edit_is_skelter_widgets_and_sends_text_runs() {
    use skelter_base::accesskit::Role;
    use skelter_base::geom::{Scale, Size};
    use skelter_base::ids::WindowId;
    use skelter_base::time::Time;
    let r = Rig::new();
    assert!(r.backend.kinds().by_tag("text-edit").unwrap().hosted, "not a placeholder");
    let col = r.create("box");
    r.backend.attach_presented(col, WindowId(1), Size::new(300.0, 200.0), Scale(1.0)).unwrap();
    let t = r.create("text-edit");
    r.set(t, "text", Value::Str("hello".into()));
    r.insert(col, t);
    let tn = r.backend.node(t).unwrap();
    let u = r.backend.with_doc(|d| d.frame(WindowId(1), Time(0))).and_then(|c| c.a11y).expect("an accesskit update");
    let node = |id: u64| u.nodes.iter().find(|(n, _)| n.0 == id).map(|(_, n)| n);
    let field = node(tn.0).expect("the field's node");
    assert_eq!(field.role(), Role::TextInput);
    assert_eq!(field.value(), Some("hello"));
    let runs: Vec<_> = field.children().iter().filter_map(|c| node(c.0)).filter(|n| n.role() == Role::TextRun).collect();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].value(), Some("hello"));
    assert!(field.text_selection().is_some(), "the caret names a run");
}
