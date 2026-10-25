use super::*;

const PANEL: &str = r#"<script>
  let { rows, cancel } = $props();
</script>

<box anchor="jobs" flex-grow={1}>
  <label class="title" text="Jobs" />
  {#if rows.length === 0}
    <label text="Nothing running" />
  {:else}
    {#each rows as job (job.id)}
      <box
        gap={12}
        align-items="center"
        padding={2}
      >
        <progress fraction={job.progress < 0 ? 0 : job.progress} />
        <button text={`Cancel ${job.name}`} onclicked={() => cancel(job.id)} disabled />
      </box>
    {/each}
  {/if}
  <Row {...rest} gap={4} />
</box>

<style>
  .title {
    color: var(--sv-text);
    font-size: 14px;
  }
  box > label { color: red; }
  .empty {}
  @media (min-width: 10px) {
    .wide { gap: 4px; }
  }
</style>
"#;

fn at(src: &str, needle: &str) -> u32 {
    src.find(needle).unwrap_or_else(|| panic!("{needle} not in source")) as u32
}

fn node<'d>(d: &'d Design, needle: &str) -> &'d DesignNode {
    d.node_at(at(d.source(), needle)).unwrap_or_else(|| panic!("no node at {needle}"))
}

fn attr<'n>(n: &'n DesignNode, name: &str) -> &'n DesignAttr {
    n.attrs.iter().find(|a| a.name == name).unwrap_or_else(|| panic!("no {name}"))
}

fn plan(src: &str, intent: Intent) -> Result<Plan, Refusal> {
    Design::parse(src).expect("parses").plan(&intent)
}

fn set(node: u32, name: &str, value: Literal) -> Intent {
    Intent::SetAttr { node, name: name.into(), value }
}

#[test]
fn the_outline_classifies_nodes_and_attributes() {
    let d = Design::parse(PANEL).unwrap();
    let outer = node(&d, "<box anchor");
    assert_eq!((outer.kind, outer.class), (NodeKind::Element, Editability::Editable));
    assert_eq!(attr(outer, "anchor").literal, Some(Literal::Str("jobs".into())));
    assert_eq!(attr(outer, "flex-grow").literal, Some(Literal::Num(1.0)));
    assert_eq!(d.roots, vec![d.nodes.iter().position(|n| n.start == outer.start).unwrap()]);

    let iff = node(&d, "{#if");
    assert_eq!((iff.kind, iff.class, iff.why), (NodeKind::If, Editability::Structural, Some(Why::Block)));
    let branches: Vec<Branch> = iff.children.iter().map(|&c| d.nodes[c].branch).collect();
    assert_eq!(branches, vec![Branch::Body, Branch::Else]);
    assert_eq!(node(&d, "{#each").path, vec![0, 1, 1]);

    let row = node(&d, "<box\n");
    assert_eq!(attr(row, "gap").literal, Some(Literal::Num(12.0)));
    let progress = node(&d, "<progress");
    assert_eq!(attr(progress, "fraction").why, Some(Why::Expression));
    let button = node(&d, "<button");
    assert_eq!(attr(button, "text").why, Some(Why::Expression));
    assert_eq!(attr(button, "onclicked").why, Some(Why::Binding));
    assert_eq!(attr(button, "disabled").literal, Some(Literal::Bool(true)));
    let spread = node(&d, "<Row");
    assert_eq!((spread.kind, spread.class, spread.why), (NodeKind::Component, Editability::Structural, Some(Why::Spread)));
    // a node at an offset inside the button finds the button
    assert_eq!(d.node_containing(at(PANEL, "Cancel")).map(|n| n.start), Some(button.start));
}

#[test]
fn the_outline_classifies_rules() {
    let d = Design::parse(PANEL).unwrap();
    let by = |sel: &str| d.rules.iter().find(|r| r.selector == sel).unwrap_or_else(|| panic!("no rule {sel}"));
    let title = by(".title");
    assert_eq!(title.class, Editability::Editable);
    assert_eq!(title.decls.iter().map(|x| (x.property.as_str(), x.value.as_str())).collect::<Vec<_>>(), vec![("color", "var(--sv-text)"), ("font-size", "14px")]);
    assert_eq!(&PANEL[title.decls[1].value_start as usize..title.decls[1].end as usize], "14px");
    assert_eq!(by("box > label").why, Some(Why::Selector));
    assert_eq!(by(".wide").why, Some(Why::AtRule));
    assert_eq!(d.rule_at(at(PANEL, ".title {")).map(|r| r.selector.as_str()), Some(".title"));
}

#[test]
fn setting_an_attribute_replaces_its_value_in_place() {
    let n = at(PANEL, "<box\n");
    let p = plan(PANEL, set(n, "gap", Literal::Num(16.0))).unwrap();
    assert_eq!(p.edits.len(), 1);
    assert!(p.text.contains("gap={16}") && !p.text.contains("gap={12}"));
    assert_eq!(p.text.len(), PANEL.len());
    // quoted text keeps its quote; a value with `{` becomes a JS string
    let p = plan(PANEL, set(n, "align-items", Literal::Str("start".into()))).unwrap();
    assert!(p.text.contains("align-items=\"start\""));
    let p = plan(PANEL, set(n, "align-items", Literal::Str("a {b}".into()))).unwrap();
    assert!(p.text.contains("align-items={\"a {b}\"}"), "{}", p.text);
    // the same value is no edit
    assert!(plan(PANEL, set(n, "gap", Literal::Num(12.0))).unwrap().edits.is_empty());
    // a bare attribute set to false gets a value
    let b = at(PANEL, "<button");
    let p = plan(PANEL, set(b, "disabled", Literal::Bool(false))).unwrap();
    assert!(p.text.contains("disabled={false} />"), "{}", p.text);
}

#[test]
fn a_new_attribute_follows_the_tags_separator() {
    // one attribute per line: a new line at the same indent
    let p = plan(PANEL, set(at(PANEL, "<box\n"), "flex-wrap", Literal::Str("wrap".into()))).unwrap();
    assert!(p.text.contains("        padding={2}\n        flex-wrap=\"wrap\"\n      >"), "{}", p.text);
    // on one line: a space
    let p = plan(PANEL, set(at(PANEL, "<box anchor"), "gap", Literal::Num(8.0))).unwrap();
    assert!(p.text.contains("<box anchor=\"jobs\" flex-grow={1} gap={8}>"), "{}", p.text);
    // no attributes yet: after the name
    let src = "<box>\n  <label />\n</box>\n";
    let p = plan(src, set(0, "padding", Literal::Num(4.0))).unwrap();
    assert_eq!(p.text, "<box padding={4}>\n  <label />\n</box>\n");
    // single-quoted tags get single quotes
    let src = "<box anchor='a'></box>\n";
    let p = plan(src, set(0, "name", Literal::Str("x".into()))).unwrap();
    assert_eq!(p.text, "<box anchor='a' name='x'></box>\n");
    // a style directive
    let p = plan(src, set(0, "style:color", Literal::Str("red".into()))).unwrap();
    assert_eq!(p.text, "<box anchor='a' style:color='red'></box>\n");
    let p = plan(&p.text, set(0, "style:color", Literal::Str("blue".into()))).unwrap();
    assert_eq!(p.text, "<box anchor='a' style:color='blue'></box>\n");
}

#[test]
fn clearing_an_attribute_takes_its_separator() {
    let row = at(PANEL, "<box\n");
    let p = plan(PANEL, Intent::ClearAttr { node: row, name: "align-items".into() }).unwrap();
    assert!(p.text.contains("<box\n        gap={12}\n        padding={2}\n      >"), "{}", p.text);
    let src = "<box a=\"1\" b={2} c></box>";
    let clear = |name: &str| plan(src, Intent::ClearAttr { node: 0, name: name.into() }).unwrap().text;
    assert_eq!(clear("a"), "<box b={2} c></box>");
    assert_eq!(clear("b"), "<box a=\"1\" c></box>");
    assert_eq!(clear("c"), "<box a=\"1\" b={2}></box>");
    let r = plan(src, Intent::ClearAttr { node: 0, name: "d".into() }).unwrap_err();
    assert_eq!(r.kind, RefusalKind::Missing);
}

#[test]
fn code_is_refused_with_its_span() {
    let r = plan(PANEL, set(at(PANEL, "<progress"), "fraction", Literal::Num(0.5))).unwrap_err();
    assert_eq!(r.kind, RefusalKind::NotEditable(Why::Expression));
    let (s, e) = r.span.unwrap();
    assert!(PANEL[s as usize..e as usize].starts_with("fraction={job.progress"));
    let r = plan(PANEL, Intent::ClearAttr { node: at(PANEL, "<button"), name: "onclicked".into() }).unwrap_err();
    assert_eq!(r.kind, RefusalKind::NotEditable(Why::Binding));
    let r = plan(PANEL, set(at(PANEL, "<Row"), "gap", Literal::Num(1.0))).unwrap_err();
    assert_eq!(r.kind, RefusalKind::NotEditable(Why::Spread));
    let r = plan(PANEL, set(at(PANEL, "{#if"), "gap", Literal::Num(1.0))).unwrap_err();
    assert_eq!(r.kind, RefusalKind::NotEditable(Why::Block));
    let r = plan(PANEL, set(at(PANEL, "<box anchor") + 1, "gap", Literal::Num(1.0))).unwrap_err();
    assert_eq!(r.kind, RefusalKind::NoNode);
    for bad in ["on:click", "a b", "", "x=\"y\""] {
        let r = plan(PANEL, set(at(PANEL, "<box anchor"), bad, Literal::Num(1.0))).unwrap_err();
        assert_eq!(r.kind, RefusalKind::BadValue, "{bad}");
    }
    let r = plan(PANEL, set(at(PANEL, "<box anchor"), "gap", Literal::Num(f64::NAN))).unwrap_err();
    assert_eq!(r.kind, RefusalKind::BadValue);
    assert_eq!(Design::parse("<box").unwrap_err().kind, RefusalKind::Parse);
    assert_eq!(Design::parse("\u{feff}<box />").unwrap_err().kind, RefusalKind::Parse);
}

#[test]
fn declarations_are_set_added_and_cleared_in_place() {
    let title = at(PANEL, ".title {");
    let decl = |property: &str, value: &str| Intent::SetDecl { rule: title, property: property.into(), value: value.into() };
    let p = plan(PANEL, decl("font-size", "16px")).unwrap();
    assert!(p.text.contains("    font-size: 16px;\n  }"), "{}", p.text);
    let p = plan(PANEL, decl("font-weight", "bold")).unwrap();
    assert!(p.text.contains("    font-size: 14px;\n    font-weight: bold;\n  }"), "{}", p.text);
    let p = plan(PANEL, Intent::ClearDecl { rule: title, property: "color".into() }).unwrap();
    assert!(p.text.contains("  .title {\n    font-size: 14px;\n  }"), "{}", p.text);
    // an empty block gets an indented line
    let empty = at(PANEL, ".empty");
    let p = plan(PANEL, Intent::SetDecl { rule: empty, property: "gap".into(), value: "2px".into() }).unwrap();
    assert!(p.text.contains("  .empty {\n    gap: 2px;\n  }"), "{}", p.text);
    // one line without a final `;`
    let src = "<box />\n<style>\n  .a { color: red }\n</style>\n";
    let a = at(src, ".a");
    let p = plan(src, Intent::SetDecl { rule: a, property: "gap".into(), value: "1px".into() }).unwrap();
    assert_eq!(p.text, "<box />\n<style>\n  .a { color: red; gap: 1px }\n</style>\n");
    let r = plan(PANEL, Intent::SetDecl { rule: at(PANEL, "box > label"), property: "color".into(), value: "blue".into() }).unwrap_err();
    assert_eq!(r.kind, RefusalKind::NotEditable(Why::Selector));
    for bad in ["red; color: blue", "", "a { b }"] {
        assert_eq!(plan(PANEL, decl("color", bad)).unwrap_err().kind, RefusalKind::BadValue, "{bad}");
    }
}

#[test]
fn rebase_maps_offsets_through_edits() {
    let e = |start, end, text: &str| TextEdit { start, end, text: text.into() };
    let edits = [e(2, 4, "xyz"), e(10, 10, "ab")];
    assert_eq!(rebase(0, &edits), Some(0));
    assert_eq!(rebase(2, &edits), None);
    assert_eq!(rebase(3, &edits), None);
    assert_eq!(rebase(4, &edits), Some(5));
    assert_eq!(rebase(10, &edits), Some(13));
    assert_eq!(rebase(11, &edits), Some(14));
    // a planned edit names the target's new offset
    let p = plan(PANEL, set(at(PANEL, "<box anchor"), "gap", Literal::Num(8.0))).unwrap();
    let row = at(PANEL, "<box\n");
    let moved = rebase(row, &p.edits).unwrap();
    assert!(p.text[moved as usize..].starts_with("<box\n"));
    assert_eq!(p.target, at(PANEL, "<box anchor"));
}

// ---- DZR2: structural intents ----

const SIMPLE: &str = r#"<script>
  let { c } = $props();
</script>

<box class="row">
  <label text="a" />
  <label text="b" />
  <box />
</box>

<style>
  .row {
    gap: 4px;
  }

  label {
    color: red;
  }
</style>
"#;

fn ok(src: &str, intent: Intent) -> Plan {
    let p = plan(src, intent.clone()).unwrap_or_else(|r| panic!("{intent:?}: {r:?}"));
    assert_eq!(apply(src, &p.edits), p.text, "the edits make the text");
    p
}

fn refused(src: &str, intent: Intent) -> RefusalKind {
    plan(src, intent).expect_err("refused").kind
}

fn insert(parent: Option<u32>, index: u32, markup: &str) -> Intent {
    Intent::Insert { parent, index, markup: markup.into() }
}

#[test]
fn insert_follows_the_siblings_layout() {
    let outer = at(SIMPLE, "<box class");
    // at index 0 and at the end: lines of their own at the children's indent
    let p = ok(SIMPLE, insert(Some(outer), 0, "<spinner />"));
    assert!(p.text.contains("<box class=\"row\">\n  <spinner />\n  <label text=\"a\" />"), "{}", p.text);
    assert!(p.text[p.target as usize..].starts_with("<spinner />"));
    let p = ok(SIMPLE, insert(Some(outer), 3, "  <spinner />\n"));
    assert!(p.text.contains("  <box />\n  <spinner />\n</box>"), "{}", p.text);
    assert!(p.text[p.target as usize..].starts_with("<spinner />"));
    // into a self-closing box: it opens and closes, the child one step in
    let p = ok(SIMPLE, insert(Some(at(SIMPLE, "<box />")), 0, "<button text=\"go\" />"));
    assert!(p.text.contains("  <box>\n    <button text=\"go\" />\n  </box>\n</box>"), "{}", p.text);
    assert!(p.text[p.target as usize..].starts_with("<button"));
    // multi-line markup keeps its own relative indent
    let p = ok(SIMPLE, insert(Some(outer), 1, "<box\n  gap={2}\n/>"));
    assert!(p.text.contains("  <label text=\"a\" />\n  <box\n    gap={2}\n  />\n  <label text=\"b\" />"), "{}", p.text);
    // top level, after the last node
    let p = ok(SIMPLE, insert(None, 1, "<label />"));
    assert!(p.text.contains("</box>\n<label />\n\n<style>"), "{}", p.text);
    // an empty element and an empty template
    let p = ok("<box></box>\n", insert(Some(0), 0, "<label />"));
    assert_eq!(p.text, "<box>\n  <label />\n</box>\n");
    let p = ok("<script>\n  let a = 1;\n</script>\n", insert(None, 0, "<box />"));
    assert_eq!(p.text, "<script>\n  let a = 1;\n</script>\n\n<box />\n");
    assert_eq!(p.target, at(&p.text, "<box />"));
    let p = ok("<style>\n  .a { color: red; }\n</style>\n", insert(None, 0, "<box />"));
    assert_eq!(p.text, "<box />\n\n<style>\n  .a { color: red; }\n</style>\n");
    // siblings on one line: the same line, with their separator
    let p = ok("<box><label /> <label /></box>\n", insert(Some(0), 1, "<x />"));
    assert_eq!(p.text, "<box><label /> <x /> <label /></box>\n");
    let p = ok("<box><label /> <label /></box>\n", insert(Some(0), 2, "<x />"));
    assert_eq!(p.text, "<box><label /> <label /> <x /></box>\n");
    // the file's indent step (4 spaces)
    let src = "<box>\n    <label />\n    <box />\n</box>\n";
    let p = ok(src, insert(Some(at(src, "<box />")), 0, "<x />"));
    assert_eq!(p.text, "<box>\n    <label />\n    <box>\n        <x />\n    </box>\n</box>\n");
}

#[test]
fn insert_refusals() {
    assert_eq!(refused(PANEL, insert(Some(at(PANEL, "{#if")), 0, "<label />")), RefusalKind::NotEditable(Why::Block));
    assert_eq!(refused(SIMPLE, insert(Some(at(SIMPLE, "<label text=\"a\"") + 1), 0, "<label />")), RefusalKind::NoNode);
    for bad in ["<a /><b />", "text", "{x}", "<box", "", "{#if a}<b />{/if}"] {
        assert_eq!(refused(SIMPLE, insert(None, 0, bad)), RefusalKind::BadValue, "{bad}");
    }
    assert_eq!(refused(SIMPLE, insert(Some(at(SIMPLE, "<box class")), 4, "<x />")), RefusalKind::BadValue);
    // a spread parent takes children
    ok(PANEL, insert(Some(at(PANEL, "<Row")), 0, "<label />"));
}

#[test]
fn remove_takes_the_line() {
    let p = ok(SIMPLE, Intent::Remove { node: at(SIMPLE, "<label text=\"b\"") });
    assert!(p.text.contains("  <label text=\"a\" />\n  <box />\n</box>"), "{}", p.text);
    assert_eq!(p.target, at(SIMPLE, "<box class"));
    // the only child
    let p = ok("<box>\n  <label />\n</box>\n", Intent::Remove { node: 8 });
    assert_eq!(p.text, "<box>\n</box>\n");
    assert_eq!(p.target, 0);
    // a subtree, at the top level
    let p = ok(SIMPLE, Intent::Remove { node: at(SIMPLE, "<box class") });
    assert!(p.text.contains("</script>\n\n\n<style>"), "{}", p.text);
    assert_eq!(p.target, 0);
    // on a shared line
    let p = ok("<box><a /> <b /></box>\n", Intent::Remove { node: 11 });
    assert_eq!(p.text, "<box><a /></box>\n");
    // a block is structural: it can go
    let p = ok(PANEL, Intent::Remove { node: at(PANEL, "{#if") });
    assert!(!p.text.contains("{#if") && p.text.contains("<label class=\"title\" text=\"Jobs\" />\n  <Row"), "{}", p.text);
    assert_eq!(refused(SIMPLE, Intent::Remove { node: 3 }), RefusalKind::NoNode);
}

#[test]
fn move_reorders_and_reparents() {
    let a = at(SIMPLE, "<label text=\"a\"");
    let b = at(SIMPLE, "<label text=\"b\"");
    let outer = Some(at(SIMPLE, "<box class"));
    // up
    let p = ok(SIMPLE, Intent::Move { node: b, parent: outer, index: 0 });
    assert!(p.text.contains("<box class=\"row\">\n  <label text=\"b\" />\n  <label text=\"a\" />\n  <box />\n</box>"), "{}", p.text);
    assert!(p.text[p.target as usize..].starts_with("<label text=\"b\""));
    // down
    let p = ok(SIMPLE, Intent::Move { node: a, parent: outer, index: 1 });
    assert!(p.text.contains("<box class=\"row\">\n  <label text=\"b\" />\n  <label text=\"a\" />\n  <box />\n</box>"), "{}", p.text);
    assert!(p.text[p.target as usize..].starts_with("<label text=\"a\""));
    let p = ok(SIMPLE, Intent::Move { node: a, parent: outer, index: 2 });
    assert!(p.text.contains("  <label text=\"b\" />\n  <box />\n  <label text=\"a\" />\n</box>"), "{}", p.text);
    // where it is already: no edit
    assert!(ok(SIMPLE, Intent::Move { node: a, parent: outer, index: 0 }).edits.is_empty());
    // into another parent
    let p = ok(SIMPLE, Intent::Move { node: a, parent: Some(at(SIMPLE, "<box />")), index: 0 });
    assert!(p.text.contains("<box class=\"row\">\n  <label text=\"b\" />\n  <box>\n    <label text=\"a\" />\n  </box>\n</box>"), "{}", p.text);
    assert!(p.text[p.target as usize..].starts_with("<label text=\"a\""));
    // out to the top level
    let p = ok(SIMPLE, Intent::Move { node: a, parent: None, index: 0 });
    assert!(p.text.contains("</script>\n\n<label text=\"a\" />\n<box class=\"row\">\n  <label text=\"b\" />"), "{}", p.text);
    // into itself or its own descendant
    let o = at(SIMPLE, "<box class");
    assert_eq!(refused(SIMPLE, Intent::Move { node: o, parent: Some(at(SIMPLE, "<box />")), index: 0 }), RefusalKind::BadValue);
    assert_eq!(refused(SIMPLE, Intent::Move { node: o, parent: Some(o), index: 0 }), RefusalKind::BadValue);
    // a block parent is refused
    assert_eq!(refused(PANEL, Intent::Move { node: at(PANEL, "<Row"), parent: Some(at(PANEL, "{#each")), index: 0 }), RefusalKind::NotEditable(Why::Block));
}

#[test]
fn wrap_and_unwrap() {
    let a = at(SIMPLE, "<label text=\"a\"");
    let b = at(SIMPLE, "<label text=\"b\"");
    let wrap = |first, last| Intent::Wrap { first, last, tag: "box".into(), attrs: vec![("gap".into(), Literal::Num(2.0)), ("anchor".into(), Literal::Str("x".into()))] };
    let p = ok(SIMPLE, wrap(a, a));
    assert!(p.text.contains("<box class=\"row\">\n  <box gap={2} anchor=\"x\">\n    <label text=\"a\" />\n  </box>\n  <label text=\"b\" />"), "{}", p.text);
    assert!(p.text[p.target as usize..].starts_with("<box gap={2}"));
    let p = ok(SIMPLE, wrap(a, b));
    assert!(p.text.contains("  <box gap={2} anchor=\"x\">\n    <label text=\"a\" />\n    <label text=\"b\" />\n  </box>\n  <box />"), "{}", p.text);
    // and back
    let q = ok(&p.text, Intent::Unwrap { node: p.target });
    assert_eq!(q.text, SIMPLE);
    assert!(q.text[q.target as usize..].starts_with("<label text=\"a\""));
    // inline siblings wrap inline
    let src = "<box><a /> <b /> <c /></box>\n";
    let p = ok(src, Intent::Wrap { first: 5, last: 11, tag: "row".into(), attrs: vec![] });
    assert_eq!(p.text, "<box><row><a /> <b /></row> <c /></box>\n");
    assert_eq!(p.target, 5);
    let q = ok(&p.text, Intent::Unwrap { node: 5 });
    assert_eq!(q.text, src);
    // unwrap the outer box: its children move out one step
    let p = ok(SIMPLE, Intent::Unwrap { node: at(SIMPLE, "<box class") });
    assert!(p.text.contains("</script>\n\n<label text=\"a\" />\n<label text=\"b\" />\n<box />\n\n<style>"), "{}", p.text);
    // refusals
    assert_eq!(refused(SIMPLE, wrap(b, a)), RefusalKind::BadValue);
    assert_eq!(refused(SIMPLE, wrap(a, at(SIMPLE, "<box class"))), RefusalKind::BadValue);
    assert_eq!(refused(SIMPLE, Intent::Wrap { first: a, last: a, tag: "a b".into(), attrs: vec![] }), RefusalKind::BadValue);
    assert_eq!(refused(SIMPLE, Intent::Unwrap { node: at(SIMPLE, "<box />") }), RefusalKind::BadValue);
    assert_eq!(refused(PANEL, Intent::Unwrap { node: at(PANEL, "<Row") }), RefusalKind::BadValue);
    assert_eq!(refused(PANEL, Intent::Unwrap { node: at(PANEL, "{#if") }), RefusalKind::NotEditable(Why::Block));
    // the each block's branch siblings
    let p = ok(PANEL, wrap(at(PANEL, "<progress"), at(PANEL, "<button")));
    assert!(p.text.contains("        <box gap={2} anchor=\"x\">\n          <progress"), "{}", p.text);
}

#[test]
fn add_class_writes_the_static_class() {
    let add = |node, class: &str| Intent::AddClass { node, class: class.into() };
    let p = ok(SIMPLE, add(at(SIMPLE, "<label text=\"a\""), "hot"));
    assert!(p.text.contains("<label text=\"a\" class=\"hot\" />"), "{}", p.text);
    assert_eq!(p.target, at(SIMPLE, "<label text=\"a\""));
    let p = ok(SIMPLE, add(at(SIMPLE, "<box class"), "hot"));
    assert!(p.text.contains("<box class=\"row hot\">"), "{}", p.text);
    assert!(ok(SIMPLE, add(at(SIMPLE, "<box class"), "row")).edits.is_empty());
    let src = "<box class={c} />\n<box class=\"a {c}\" />\n";
    assert_eq!(refused(src, add(0, "x")), RefusalKind::NotEditable(Why::Expression));
    assert_eq!(refused(src, add(at(src, "<box class=\""), "x")), RefusalKind::NotEditable(Why::Interpolated));
    for bad in ["1x", "a b", "", "x.y"] {
        assert_eq!(refused(SIMPLE, add(at(SIMPLE, "<box class"), bad)), RefusalKind::BadValue, "{bad}");
    }
}

#[test]
fn add_rule_follows_the_sheets_layout() {
    let rule = |sel: &str, decls: &[(&str, &str)]| Intent::AddRule { selector: sel.into(), decls: decls.iter().map(|(p, v)| (p.to_string(), v.to_string())).collect() };
    // blank lines between rules, multi-line blocks
    let p = ok(SIMPLE, rule(".hot:hover", &[("color", "blue"), ("gap", "2px")]));
    assert!(p.text.ends_with("  label {\n    color: red;\n  }\n\n  .hot:hover {\n    color: blue;\n    gap: 2px;\n  }\n</style>\n"), "{}", p.text);
    assert_eq!(Design::parse(&p.text).unwrap().rule_at(p.target).map(|r| r.selector.as_str()), Some(".hot:hover"));
    // no blank line, a 4-space step
    let src = "<box />\n<style>\n    .a {\n        color: red;\n    }\n</style>\n";
    let p = ok(src, rule("box:state(cad.pending)", &[("gap", "1px")]));
    assert_eq!(p.text, "<box />\n<style>\n    .a {\n        color: red;\n    }\n    box:state(cad.pending) {\n        gap: 1px;\n    }\n</style>\n");
    // no <style>: one at the end
    let p = ok("<box />\n", rule(".x", &[("color", "blue")]));
    assert_eq!(p.text, "<box />\n\n<style>\n  .x {\n    color: blue;\n  }\n</style>\n");
    assert_eq!(p.target, at(&p.text, ".x"));
    // an empty <style>
    let p = ok("<box />\n<style>\n</style>\n", rule(".x", &[("color", "blue")]));
    assert_eq!(p.text, "<box />\n<style>\n  .x {\n    color: blue;\n  }\n</style>\n");
    // refusals
    assert_eq!(refused(SIMPLE, rule("box > label", &[("color", "red")])), RefusalKind::NotEditable(Why::Selector));
    assert_eq!(refused(SIMPLE, rule(":global(.a)", &[("color", "red")])), RefusalKind::NotEditable(Why::Selector));
    for bad in [".a {", "", ".a; .b"] {
        assert_eq!(refused(SIMPLE, rule(bad, &[("color", "red")])), RefusalKind::BadValue, "{bad}");
    }
    assert_eq!(refused(SIMPLE, rule(".a", &[("color", "red; x: y")])), RefusalKind::BadValue);
    assert_eq!(refused(SIMPLE, rule(".a", &[("1x", "red")])), RefusalKind::BadValue);
}

#[test]
fn plan_all_is_one_plan_in_the_original_offsets() {
    let d = Design::parse(SIMPLE).unwrap();
    let a = at(SIMPLE, "<label text=\"a\"");
    let intents = [Intent::AddClass { node: a, class: "hot".into() }, Intent::AddRule { selector: ".hot".into(), decls: vec![("color".into(), "blue".into())] }];
    let p = d.plan_all(&intents).unwrap();
    assert_eq!(p.edits.len(), 2, "{:?}", p.edits);
    assert!(p.edits.windows(2).all(|w| w[0].end <= w[1].start));
    assert_eq!(apply(SIMPLE, &p.edits), p.text);
    assert!(p.text.contains("<label text=\"a\" class=\"hot\" />") && p.text.contains("  .hot {\n    color: blue;\n  }\n</style>"), "{}", p.text);
    assert!(p.text[p.target as usize..].starts_with(".hot {"));
    // a later step that edits what an earlier one wrote: one edit over the changed range
    let intents = [insert(Some(at(SIMPLE, "<box />")), 0, "<x />"), Intent::AddClass { node: at(SIMPLE, "<box />"), class: "k".into() }];
    let p = d.plan_all(&intents).unwrap();
    assert_eq!(apply(SIMPLE, &p.edits), p.text);
    assert!(p.text.contains("  <box class=\"k\">\n    <x />\n  </box>"), "{}", p.text);
    // a refused step refuses the whole
    let intents = [Intent::AddClass { node: a, class: "hot".into() }, Intent::AddRule { selector: "a b".into(), decls: vec![] }];
    assert_eq!(d.plan_all(&intents).unwrap_err().kind, RefusalKind::NotEditable(Why::Selector));
    assert!(d.plan_all(&[]).unwrap().edits.is_empty());
}
