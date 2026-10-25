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
