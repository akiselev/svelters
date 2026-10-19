//! Unit tests for the parser port. Correctness is defined by the oracle
//! (`tests/oracle_svelte.rs`); these pin behaviour that does not need acorn, with expectations
//! taken from `svelte/compiler` 5.57.2. Tests that need JS expressions are ignored until the
//! acorn port lands.

use super::*;
use crate::svelte::ast::{AttributeLike, AttributeValue, FragmentNode as N, TextOrExpressionTag};
use serde_json::{Value, json};

fn opts() -> ParseOptions {
    ParseOptions::default()
}

fn native() -> ParseOptions {
    ParseOptions { native: true, ..Default::default() }
}

fn ok(src: &str) -> Root {
    parse(src, &opts()).unwrap_or_else(|e| panic!("{src:?}: {e:?}"))
}

fn json_of(src: &str, o: &ParseOptions) -> Value {
    serde_json::to_value(parse(src, o).unwrap_or_else(|e| panic!("{src:?}: {e:?}"))).unwrap()
}

fn err_with(src: &str, o: &ParseOptions) -> (&'static str, String, u32, u32) {
    match parse(src, o) {
        Ok(_) => panic!("{src:?}: expected an error"),
        Err(e) => (e.code, e.message, e.start, e.end),
    }
}

fn err(src: &str) -> (&'static str, String, u32, u32) {
    err_with(src, &opts())
}

fn warnings(src: &str, o: &ParseOptions) -> Vec<(&'static str, u32, u32)> {
    parse_with_warnings(src, o).unwrap().1.into_iter().map(|w| (w.code, w.start, w.end)).collect()
}

fn names(nodes: &[N]) -> Vec<String> {
    nodes
        .iter()
        .map(|n| match n {
            N::RegularElement(e) => format!("<{}>", e.name),
            N::Text(t) => format!("{:?}", t.data),
            N::Comment(c) => format!("<!--{}-->", c.data),
            other => serde_json::to_value(other).unwrap()["type"].as_str().unwrap().to_string(),
        })
        .collect()
}

fn element(n: &N) -> &ast::Element {
    match n {
        N::RegularElement(e) | N::Component(e) | N::SvelteHead(e) | N::TitleElement(e) | N::SlotElement(e) => e,
        other => panic!("not an element: {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// text and entities

#[test]
fn text_and_entities() {
    let root = ok("a &amp; b &lt;&gt; &#65;&#x42; &copy &unknown; &#0; &#10;");
    let N::Text(t) = &root.fragment.nodes[0] else { panic!() };
    assert_eq!(t.raw, "a &amp; b &lt;&gt; &#65;&#x42; &copy &unknown; &#0; &#10;");
    // `&#0;` is kept (code 0 is falsy), `&#10;` becomes a space outside attributes
    assert_eq!(t.data, "a & b <> AB © &unknown; &#0;  ");
    assert_eq!((t.start, t.end), (0, 57));
}

#[test]
fn entities_windows_1252_and_invalid_code_points() {
    let root = ok("&#128;&#x9f;&#xD800;&#1114112;&#99999999999999999999;");
    let N::Text(t) = &root.fragment.nodes[0] else { panic!() };
    assert_eq!(t.data, "\u{20ac}\u{178}\0\0\0");
}

#[test]
fn entities_in_attribute_values() {
    // without `;`, a named reference is not decoded in an attribute when followed by `=` or an
    // alphanumeric character
    let root = ok("<a href=\"?a=1&copy=2&amp=3&copyx&copy;&amp\"></a>");
    let el = element(&root.fragment.nodes[0]);
    let AttributeLike::Attribute(a) = &el.attributes[0] else { panic!() };
    let AttributeValue::Sequence(v) = &a.value else { panic!() };
    let TextOrExpressionTag::Text(t) = &v[0] else { panic!() };
    assert_eq!(t.data, "?a=1&copy=2&amp=3&copyx©&");
    // in text the same characters are decoded greedily
    let root = ok("&copyx");
    let N::Text(t) = &root.fragment.nodes[0] else { panic!() };
    assert_eq!(t.data, "©x");
}

#[test]
fn longest_entity_name_wins() {
    let root = ok("&notin; &not; &notit");
    let N::Text(t) = &root.fragment.nodes[0] else { panic!() };
    assert_eq!(t.data, "∉ ¬ ¬it");
}

#[test]
fn non_ascii_positions_are_bytes() {
    let root = ok("é<b>ü</b>");
    let N::Text(t) = &root.fragment.nodes[0] else { panic!() };
    assert_eq!((t.start, t.end), (0, 2));
    let b = element(&root.fragment.nodes[1]);
    assert_eq!((b.start, b.end), (2, 11));
    assert_eq!(root.end, 11);
}

#[test]
fn bom_is_removed_and_trailing_whitespace_trimmed() {
    let root = ok("\u{feff}<p></p>  \n\u{3000}");
    assert_eq!(root.end, 13); // length without the BOM, including the trailing whitespace
    assert_eq!(names(&root.fragment.nodes), ["<p>"]);
}

// ---------------------------------------------------------------------------------------------
// elements, comments, attributes

#[test]
fn comments() {
    let root = ok("<!-- a --><div><!--b--></div>");
    assert_eq!(names(&root.fragment.nodes), ["<!-- a -->", "<div>"]);
    assert_eq!(err("<!-- a"), ("expected_token", "Expected token -->".into(), 6, 6));
}

#[test]
fn nested_elements_and_spans() {
    let root = ok("<div class=\"a\">x<span>y</span><br><img/></div>");
    let div = element(&root.fragment.nodes[0]);
    assert_eq!((div.start, div.end), (0, 46));
    assert_eq!(div.name_span, Span { start: 1, end: 4 });
    assert_eq!(names(&div.fragment.nodes), ["\"x\"", "<span>", "<br>", "<img>"]);
    let br = element(&div.fragment.nodes[2]);
    assert_eq!((br.start, br.end), (30, 34));
}

#[test]
fn static_attribute_forms() {
    let v = json_of("<input a b=c d='e' f=\"\" g={h} {i} j=\"x{k}y\" l=/>", &opts());
    let attrs = &v["fragment"]["nodes"][0]["attributes"];
    assert_eq!(attrs[0], json!({"type":"Attribute","start":7,"end":8,"name":"a","value":true}));
    assert_eq!(attrs[1]["value"], json!([{"type":"Text","start":11,"end":12,"raw":"c","data":"c"}]));
    assert_eq!(attrs[2]["value"], json!([{"type":"Text","start":16,"end":17,"raw":"e","data":"e"}]));
    assert_eq!(attrs[3]["value"], json!([{"type":"Text","start":22,"end":22,"raw":"","data":""}]));
    assert_eq!(attrs[4]["value"]["type"], "ExpressionTag");
    assert_eq!(attrs[4]["value"]["expression"]["name"], "h");
    assert_eq!(attrs[5]["name"], "i");
    assert_eq!(attrs[5]["value"]["expression"], json!({"type":"Identifier","start":31,"end":32,"name":"i"}));
    assert_eq!(attrs[6]["value"].as_array().unwrap().len(), 3);
    // `l=/>`: the `/` is the value
    assert_eq!(attrs[7]["value"], json!([{"type":"Text","start":46,"end":47,"raw":"/","data":"/"}]));
}

#[test]
fn directives() {
    let v = json_of("<div use:a bind:b let:c on:d|once|capture in:e out:f transition:g style:h|important=\"x\" class:i animate:j></div>", &opts());
    let attrs = v["fragment"]["nodes"][0]["attributes"].as_array().unwrap();
    assert_eq!(attrs[0], json!({"type":"UseDirective","start":5,"end":10,"name":"a","expression":null,"modifiers":[]}));
    assert_eq!(attrs[1]["expression"], json!({"type":"Identifier","start":16,"end":17,"name":"b"}));
    assert_eq!(attrs[3]["modifiers"], json!(["once", "capture"]));
    assert_eq!((attrs[4]["intro"].as_bool(), attrs[4]["outro"].as_bool()), (Some(true), Some(false)));
    assert_eq!((attrs[5]["intro"].as_bool(), attrs[5]["outro"].as_bool()), (Some(false), Some(true)));
    assert_eq!((attrs[6]["intro"].as_bool(), attrs[6]["outro"].as_bool()), (Some(true), Some(true)));
    assert_eq!(attrs[7]["type"], "StyleDirective");
    assert_eq!(attrs[7]["modifiers"], json!(["important"]));
    assert_eq!(attrs[8]["expression"]["name"], "i");
    assert_eq!(attrs[9]["type"], "AnimateDirective");
}

#[test]
fn attribute_errors() {
    assert_eq!(err("<div a a></div>").0, "attribute_duplicate");
    assert_eq!(err("<div a bind:a></div>").0, "attribute_duplicate");
    // class: and style: directives may coexist with an attribute of the same name
    ok("<div a class:a style:a></div>");
    assert_eq!(err("<div class:=\"x\"></div>"), ("directive_missing_name", "`class:` name cannot be empty".into(), 5, 11));
    assert_eq!(err("<div on:click=\"x\"></div>").0, "directive_invalid_value");
    assert_eq!(err("<div a\"b\"></div>"), ("expected_token", "Expected token =".into(), 6, 6));
    assert_eq!(err("<div a=></div>").0, "expected_attribute_value");
    assert_eq!(err("<div {}></div>").0, "attribute_empty_shorthand");
    assert_eq!(err("<div a=\"{#if x}\"></div>"), ("block_invalid_placement", "{#if ...} block cannot be in attribute value".into(), 8, 8));
    assert_eq!(err("<div a=\"{@html x}\"></div>").0, "tag_invalid_placement");
    assert_eq!(err("<div a=\"x").0, "unexpected_eof");
}

#[test]
fn element_structure_errors() {
    assert_eq!(err("<div>"), ("element_unclosed", "`<div>` was left open".into(), 0, 1));
    assert_eq!(err("</div>"), ("element_invalid_closing_tag", "`</div>` attempted to close an element that was not open".into(), 0, 0));
    assert_eq!(err("<br></br>"), ("void_element_invalid_content", "Void elements cannot have children or closing tags".into(), 4, 4));
    assert_eq!(err("<Foo>").0, "block_unclosed");
    assert_eq!(err("<div></span>").0, "element_invalid_closing_tag");
    assert_eq!(err("<1div></1div>"), ("tag_invalid_name", err("<1div></1div>").1, 1, 5));
    assert_eq!(err("<").0, "unexpected_eof");
    assert_eq!(err("<div").0, "unexpected_eof");
}

#[test]
fn component_and_special_element_types() {
    let root = ok("<Foo /><foo.bar /><a.b.c></a.b.c><svelte:self /><svelte:fragment></svelte:fragment><svelte:boundary></svelte:boundary><slot />");
    assert_eq!(names(&root.fragment.nodes), ["Component", "Component", "Component", "SvelteSelf", "SvelteFragment", "SvelteBoundary", "SlotElement"]);
    let root = ok("<svelte:head><title>x</title></svelte:head><title></title>");
    assert_eq!(names(&root.fragment.nodes), ["SvelteHead", "<title>"]);
    assert_eq!(names(&element(&root.fragment.nodes[0]).fragment.nodes), ["TitleElement"]);
}

#[test]
fn meta_tag_errors() {
    let (code, msg, s, e) = err("<svelte:foo />");
    assert_eq!((code, s, e), ("svelte_meta_invalid_tag", 1, 11));
    assert_eq!(
        msg,
        "Valid `<svelte:...>` tag names are svelte:head, svelte:options, svelte:window, svelte:document, svelte:body, svelte:element, svelte:component, svelte:self, svelte:fragment or svelte:boundary"
    );
    assert_eq!(err("<svelte:window /><svelte:window />"), ("svelte_meta_duplicate", "A component can only have one `<svelte:window>` element".into(), 17, 17));
    assert_eq!(err("<div><svelte:body /></div>").0, "svelte_meta_invalid_placement");
    assert_eq!(err("<svelte:component />").0, "svelte_component_missing_this");
    assert_eq!(err("<svelte:component this=\"x\" />").0, "svelte_component_invalid_this");
    assert_eq!(err("<svelte:element />").0, "svelte_element_missing_this");
    assert_eq!(err("<svelte:element this />").0, "svelte_element_missing_this");
    // only <svelte:options> children are rejected by the parser (the others: analysis)
    ok("<svelte:window>x</svelte:window>");
}

#[test]
fn svelte_element_with_string_this_warns() {
    let (root, w) = parse_with_warnings("<svelte:element this=\"div\" />", &opts()).unwrap();
    assert_eq!(w[0].code, "svelte_element_invalid_this");
    let v = serde_json::to_value(&root).unwrap();
    assert_eq!(v["fragment"]["nodes"][0]["tag"], json!({"type":"Literal","start":22,"end":25,"value":"div","raw":"'div'"}));
}

#[test]
fn implicit_closing_html() {
    let root = ok("<p>a<div>b</div>");
    assert_eq!(names(&root.fragment.nodes), ["<p>", "<div>"]);
    assert_eq!(element(&root.fragment.nodes[0]).end, 4);
    assert_eq!(warnings("<p>a<div>b</div>", &opts()), [("element_implicitly_closed", 0, 3)]);

    let root = ok("<ul><li>a<li>b</ul>");
    let ul = element(&root.fragment.nodes[0]);
    assert_eq!(names(&ul.fragment.nodes), ["<li>", "<li>"]);
    assert_eq!(warnings("<ul><li>a<li>b</ul>", &opts()).len(), 2);

    // `</div>` closes the open `<p>` implicitly
    let root = ok("<div><p>x</div>");
    assert_eq!(names(&element(&root.fragment.nodes[0]).fragment.nodes), ["<p>"]);

    assert_eq!(err("<div><p><div></div></p></div>").0, "element_invalid_closing_tag_autoclosed");
}

#[test]
fn textarea_and_nested_script_style_are_raw_text_in_html() {
    let root = ok("<textarea>a<b>{x}</TEXTAREA >");
    let ta = element(&root.fragment.nodes[0]);
    assert_eq!(names(&ta.fragment.nodes), ["\"a<b>\"", "ExpressionTag"]);
    assert_eq!(ta.end, 29);
    assert_eq!(err("<textarea>{#if x}</textarea>").0, "block_invalid_placement");

    let root = ok("<div><style>a{}</style><script>x<y</script></div>");
    let div = element(&root.fragment.nodes[0]);
    assert_eq!(names(&element(&div.fragment.nodes[1]).fragment.nodes), ["\"x<y\""]);
}

#[test]
fn top_level_style_is_captured_raw() {
    let root = ok("<p></p>\n<style lang=\"x\">\n  p { color: red }\n</style  >");
    let css = root.css.unwrap();
    assert_eq!(css.content.styles, "\n  p { color: red }\n");
    assert_eq!((css.start, css.end), (8, 54));
    assert_eq!((css.content.start, css.content.end), (24, 44));
    assert_eq!(css.attributes[0].name, "lang");
    assert_eq!(err("<style>a{}</style><style></style>").0, "style_duplicate");
    assert_eq!(err("<style>a{}").0, "expected_token");
}

#[test]
fn static_attributes_on_top_level_script_and_style() {
    // `{x}` is not an attribute on top-level <style>: read_static_attribute reads the tag name
    let root = ok("<style data-x={y}></style>");
    assert_eq!(root.css.unwrap().attributes[0].name, "data-x");
}

// ---------------------------------------------------------------------------------------------
// tags and blocks that only need simple expressions (identifiers / member chains)

#[test]
fn simple_expression_tags() {
    let v = json_of("{a}{ b.c.d }", &opts());
    let nodes = &v["fragment"]["nodes"];
    assert_eq!(nodes[0], json!({"type":"ExpressionTag","start":0,"end":3,"expression":{"type":"Identifier","start":1,"end":2,"name":"a"}}));
    assert_eq!(nodes[1]["expression"]["type"], "MemberExpression");
    assert_eq!(nodes[1]["expression"]["start"], 5);
    assert_eq!(nodes[1]["expression"]["end"], 10);
    assert_eq!(nodes[1]["end"], 12);
    assert_eq!(nodes[1]["expression"]["property"], json!({"type":"Identifier","start":9,"end":10,"name":"d"}));
}

#[test]
fn if_blocks() {
    let root = ok("{#if a}x{:else if b}y{:else}z{/if}");
    let N::IfBlock(b) = &root.fragment.nodes[0] else { panic!() };
    assert_eq!((b.start, b.end, b.elseif), (0, 34, false));
    let alt = b.alternate.as_ref().unwrap();
    let N::IfBlock(c) = &alt.nodes[0] else { panic!() };
    assert_eq!((c.start, c.end, c.elseif), (8, 34, true));
    assert_eq!(names(&c.alternate.as_ref().unwrap().nodes), ["\"z\""]);
}

#[test]
fn key_await_html_debug_blocks() {
    let root = ok("{#key k}a{/key}{#await p}w{:then v}t{:catch e}c{/await}{#await p}{:then}x{/await}{@html h}{@debug}{@debug a}");
    assert_eq!(names(&root.fragment.nodes), ["KeyBlock", "AwaitBlock", "AwaitBlock", "HtmlTag", "DebugTag", "DebugTag"]);
    let N::AwaitBlock(a) = &root.fragment.nodes[1] else { panic!() };
    assert!(a.pending.is_some() && a.then.is_some() && a.catch.is_some());
    let v = serde_json::to_value(&root).unwrap();
    assert_eq!(v["fragment"]["nodes"][1]["value"], json!({"type":"Identifier","start":33,"end":34,"name":"v"}));
    assert_eq!(v["fragment"]["nodes"][2]["value"], Value::Null);
    assert_eq!(v["fragment"]["nodes"][4]["identifiers"], json!([]));
}

#[test]
fn block_structure_errors() {
    assert_eq!(err("{#if a}"), ("block_unclosed", "Block was left open".into(), 0, 1));
    assert_eq!(err("{/if}"), ("block_unexpected_close", "Unexpected block closing tag".into(), 1, 1));
    assert_eq!(err("{#if a}{/each}"), ("expected_token", "Expected token if".into(), 9, 9));
    assert_eq!(err("{#foo}").0, "expected_block_type");
    assert_eq!(err("{@foo}").0, "expected_tag");
    assert_eq!(err("{#if a}{:elseif b}{/if}").0, "block_invalid_elseif");
    assert_eq!(err("{#if a}{:else}{:else}{/if}").0, "block_duplicate_clause");
    assert_eq!(err("{#if a}{:then}{/if}").0, "expected_token");
    assert_eq!(err("{:else}").0, "block_invalid_continuation_placement");
    assert_eq!(err("{#key a}{:else}{/key}").0, "block_invalid_continuation_placement");
    assert_eq!(err("{#await a}{:then}{:then}{/await}").0, "block_duplicate_clause");
    assert_eq!(err("<div>{/if}</div>").0, "block_unexpected_close");
    assert_eq!(err("{#if a}</div>{/if}").0, "element_invalid_closing_tag");
    assert_eq!(err("{#if a}<div>{/if}").0, "block_unexpected_close");
    assert_eq!(err("{#ifa}").0, "expected_whitespace");
    assert_eq!(err("{#if}").0, "expected_whitespace");
    assert_eq!(err("<div {if}></div>"), ("unexpected_reserved_word", "'if' is a reserved word in JavaScript and cannot be used here".into(), 6, 6));
}

#[test]
fn declaration_tag_unsupported_kinds() {
    assert_eq!(err("{var x = 1}"), ("declaration_tag_invalid_type", "Declaration tags must be `let` or `const` declarations".into(), 1, 4));
    assert_eq!(err("{enum}").0, "declaration_tag_invalid_type");
}

// ---------------------------------------------------------------------------------------------
// <svelte:options>

#[test]
fn svelte_options_without_expressions() {
    let root = ok("<svelte:options runes namespace=\"http://www.w3.org/2000/svg\" css=\"injected\" customElement=\"my-el\" />x");
    let o = root.options.unwrap();
    assert_eq!(o.runes, Some(true));
    assert_eq!(o.namespace.as_deref(), Some("svg"));
    assert_eq!(o.css.as_deref(), Some("injected"));
    assert!(o.custom_element.is_some());
    assert_eq!((o.start, o.end), (0, 100));
    assert_eq!(o.attributes.len(), 4);
    assert_eq!(names(&root.fragment.nodes), ["\"x\""]);

    // stock Svelte 5.57.2 has no `customRenderer` option (it was the retired fork's)
    assert_eq!(err("<svelte:options customRenderer=\"./r.js\" />").0, "svelte_options_unknown_attribute");
}

#[test]
fn svelte_options_errors() {
    assert_eq!(err("<svelte:options runes=\"yes\" />"), ("svelte_options_invalid_attribute_value", "Value must be true or false, if specified".into(), 16, 27));
    assert_eq!(err("<svelte:options foo />"), ("svelte_options_unknown_attribute", "`<svelte:options>` unknown attribute 'foo'".into(), 16, 19));
    assert_eq!(err("<svelte:options tag=\"x-y\" />").0, "svelte_options_deprecated_tag");
    assert_eq!(err("<svelte:options namespace=\"foo\" />").1, "Value must be \"html\", \"mathml\" or \"svg\", if specified");
    assert_eq!(err("<svelte:options css=\"external\" />").0, "svelte_options_invalid_attribute_value");
    assert_eq!(err("<svelte:options customElement=\"Foo\" />").0, "svelte_options_invalid_tagname");
    assert_eq!(err("<svelte:options customElement=\"font-face\" />").0, "svelte_options_reserved_tagname");
    assert_eq!(err("<svelte:options customElement />").0, "svelte_options_invalid_customelement");
    assert_eq!(err("<svelte:options class:x />").0, "svelte_options_invalid_attribute");
    assert_eq!(err("<svelte:options>x</svelte:options>"), ("svelte_meta_invalid_content", "<svelte:options> cannot have children".into(), 16, 17));
}

// ---------------------------------------------------------------------------------------------
// TypeScript

#[test]
fn typescript_is_detected() {
    let ts = |src: &str| parse(src, &opts()).map(|r| r.ts);
    assert_eq!(ts("<p></p><script lang=\"ts\">let x: number = 1;</script>"), Ok(true));
    assert_eq!(ts("<script context=\"module\" lang='ts'></script>"), Ok(true));
    assert_eq!(ts("<script foo=\"a>b\" lang=ts>let y = <T>(a: T) => a;</script>"), Ok(true));
    // a JS component rejects the same syntax
    assert_eq!(err("<script>let x: number = 1;</script>").0, "js_parse_error");
    // a script tag inside an HTML comment does not count; neither does another language
    assert!(parse("<!-- <script lang=\"ts\"> --><p></p>", &opts()).is_ok());
    assert!(parse("<p lang=\"ts\"></p>", &opts()).is_ok());
    assert!(regex_lang_attribute("<script lang=\"tsx\">").is_some_and(|(_, l)| l == "tsx"));
    assert!(regex_lang_attribute("<script>").is_none());
    assert!(regex_lang_attribute("<scriptlang=\"ts\">").is_none());
}

// ---------------------------------------------------------------------------------------------
// native mode

#[test]
fn native_mode_has_no_void_elements() {
    // `<input>` is an ordinary element that needs closing
    let root = parse("<input></input><br>x</br>", &native()).unwrap();
    assert_eq!(names(&root.fragment.nodes), ["<input>", "<br>"]);
    assert_eq!(names(&element(&root.fragment.nodes[1]).fragment.nodes), ["\"x\""]);
    assert_eq!(err_with("<input>", &native()).0, "element_unclosed");
}

#[test]
fn native_mode_has_no_implicit_closing() {
    let root = parse("<p><div></div></p><li><li></li></li>", &native()).unwrap();
    assert_eq!(names(&root.fragment.nodes), ["<p>", "<li>"]);
    assert_eq!(names(&element(&root.fragment.nodes[0]).fragment.nodes), ["<div>"]);
    assert!(warnings("<p><div></div></p>", &native()).is_empty());
    assert_eq!(err_with("<box><row></box>", &native()).0, "element_invalid_closing_tag");
    // html mode closes the <p> implicitly and then fails on the stray </p>
    assert_eq!(err("<p><div></div></p>").0, "element_invalid_closing_tag_autoclosed");
}

#[test]
fn native_mode_textarea_and_script_are_ordinary() {
    let root = parse("<textarea><b>x</b></textarea><box><script>a<b></b></script></box>", &native()).unwrap();
    assert_eq!(names(&element(&root.fragment.nodes[0]).fragment.nodes), ["<b>"]);
    let bx = element(&root.fragment.nodes[1]);
    assert_eq!(names(&element(&bx.fragment.nodes[0]).fragment.nodes), ["\"a\"", "<b>"]);
}

#[test]
fn native_mode_slot_in_shadowroot_template() {
    let src = "<template shadowrootmode=\"open\"><slot /></template>";
    let html = ok(src);
    assert_eq!(names(&element(&html.fragment.nodes[0]).fragment.nodes), ["<slot>"]);
    let nat = parse(src, &native()).unwrap();
    assert_eq!(names(&element(&nat.fragment.nodes[0]).fragment.nodes), ["SlotElement"]);
}

#[test]
fn native_mode_is_identical_otherwise() {
    for src in ["<Box a=\"1\" {b}><Label text=\"x &amp; y\" /></Box>", "{#if a}<x></x>{:else}y{/if}", "<svelte:options runes /><!-- c -->"] {
        assert_eq!(json_of(src, &native()), json_of(src, &opts()), "{src}");
    }
}

/// Dev aid: prints our result for each `\n----\n`-separated source in `$DUMP_IN`, for ad-hoc
/// comparison with `svelte/compiler`.
#[test]
#[ignore = "dev aid (DUMP_IN=<file>)"]
fn zz_dump() {
    let list = std::fs::read_to_string(std::env::var("DUMP_IN").unwrap()).unwrap();
    for src in list.split("\n----\n") {
        match std::panic::catch_unwind(|| parse(src, &opts())) {
            Ok(Ok(r)) => println!("OK {}", &serde_json::to_string(&r).unwrap()[..]),
            Ok(Err(e)) => println!("ERR {} {:?} [{}, {}]", e.code, e.message, e.start, e.end),
            Err(_) => println!("PANIC"),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// needs acorn

#[test]
#[ignore = "needs acorn (agent A)"]
fn expressions_via_acorn() {
    let v = json_of("{a + b}<div on:click={() => x++} {...rest} {@attach f}></div>{#each items as item, i (item.id)}{item}{:else}e{/each}", &opts());
    assert_eq!(v["fragment"]["nodes"][0]["expression"]["type"], "BinaryExpression");
    let each = &v["fragment"]["nodes"][2];
    assert_eq!(each["type"], "EachBlock");
    assert_eq!(each["index"], "i");
    assert_eq!(each["context"]["name"], "item");
    assert_eq!(each["key"]["type"], "MemberExpression");
}

#[test]
#[ignore = "needs acorn (agent A)"]
fn script_blocks() {
    let root = ok("<script module>export const a = 1;</script><script>let b = 2;</script>");
    assert!(root.module.is_some() && root.instance.is_some());
    assert_eq!(err("<script>a</script><script>b</script>").0, "script_duplicate");
    assert_eq!(err("<script context=\"x\"></script>").0, "script_invalid_context");
    assert_eq!(err("<script test></script>").0, "script_reserved_attribute");
    assert_eq!(err("<script>").0, "element_unclosed");
}

#[test]
#[ignore = "needs acorn (agent A)"]
fn const_render_snippet_declaration() {
    let root = ok("{#snippet s(a, { b })}{@const c = a}{@render s(c)}{/snippet}{let d = 1}");
    assert_eq!(names(&root.fragment.nodes), ["SnippetBlock", "DeclarationTag"]);
    assert_eq!(err("{@render a}").0, "render_tag_invalid_expression");
    assert_eq!(err("{@const a = b, c = d}").0, "const_tag_invalid_expression");
    assert_eq!(err("{@debug a.b}").0, "debug_tag_invalid_arguments");
}
