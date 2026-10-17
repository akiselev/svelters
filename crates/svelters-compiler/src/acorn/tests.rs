//! Unit tests for the acorn port. Differential coverage against real acorn lives in
//! `tests/acorn_dev.rs` (dev helper) and the oracle harness.

use super::*;
use crate::estree::*;

fn module(src: &str) -> Result<Program, ParseError> {
    let mut c = Vec::new();
    parse(src, &Options::default(), &mut c)
}

fn json(src: &str) -> serde_json::Value {
    serde_json::to_value(module(src).unwrap()).unwrap()
}

fn err(src: &str) -> String {
    module(src).unwrap_err().message
}

#[test]
fn simple_program() {
    let v = json("let a = 1 + 2;");
    assert_eq!(v["type"], "Program");
    assert_eq!(v["sourceType"], "module");
    let decl = &v["body"][0];
    assert_eq!(decl["type"], "VariableDeclaration");
    assert_eq!(decl["kind"], "let");
    assert_eq!(decl["declarations"][0]["init"]["type"], "BinaryExpression");
    assert_eq!(decl["declarations"][0]["init"]["operator"], "+");
    assert_eq!(decl["end"], 14);
}

#[test]
fn error_messages_have_acorn_positions() {
    assert_eq!(err("let a = ;"), "Unexpected token (1:8)");
    assert_eq!(err("a\n  b c"), "Unexpected token (2:4)");
    // UTF-16 columns: '𝒳' is 4 bytes / 2 UTF-16 units, 'é' 2 bytes / 1 unit.
    let e = module("'𝒳é' +").unwrap_err();
    assert_eq!(e.message, "Unexpected token (1:7)");
    assert_eq!(e.pos, 10);
    assert_eq!(err("x = 1;\r\ny = )"), "Unexpected token (2:4)");
    assert_eq!(err("export { foo }"), "Export 'foo' is not defined (1:9)");
}

#[test]
fn byte_offsets() {
    let v = json("'é'; b");
    assert_eq!(v["body"][1]["start"], 6);
    assert_eq!(v["body"][0]["expression"]["value"], "é");
}

#[test]
fn comments_in_order() {
    let mut c = Vec::new();
    parse("/* a */ x // b\n/*c*/", &Options::default(), &mut c).unwrap();
    assert_eq!(c.len(), 3);
    assert_eq!(c[0].kind, CommentKind::Block);
    assert_eq!(c[0].value, " a ");
    assert_eq!((c[0].start, c[0].end), (0, 7));
    assert_eq!(c[1].kind, CommentKind::Line);
    assert_eq!(c[1].value, " b");
    assert_eq!(c[2].value, "c");
}

#[test]
fn expression_at_and_parens() {
    let src = "<p>{(a + b) * c}</p>";
    let mut c = Vec::new();
    let opts = Options { preserve_parens: true, ..Options::default() };
    let e = parse_expression_at(src, 4, &opts, &mut c).unwrap();
    assert_eq!(e.span(), Span::new(4, 15));
    let Expression::BinaryExpression(b) = e else { panic!() };
    let BinaryLeft::Expression(l) = b.left else { panic!() };
    assert!(matches!(*l, Expression::ParenthesizedExpression(_)));
}

#[test]
fn statement_at() {
    let src = "{@const x = y + 1}";
    let mut c = Vec::new();
    // `x = y + 1}`: a semicolon is inserted before `}`, like acorn
    let s = parse_statement_at(src, 8, &Options::default(), &mut c).unwrap();
    assert_eq!(s.span(), Span::new(8, 17));
    let e = parse_statement_at("{@const x = y + 1 z}", 8, &Options::default(), &mut c).unwrap_err();
    assert_eq!(e.message, "Unexpected token (1:18)");
    let s = parse_statement_at("..const x = 1;", 2, &Options::default(), &mut c).unwrap();
    assert!(matches!(s, Statement::VariableDeclaration(_)));
}

#[test]
fn svelte_undefined_exports() {
    let mut c = Vec::new();
    let opts = Options { allow_undefined_exports: true, ..Options::default() };
    assert!(parse("export { foo }", &opts, &mut c).is_ok());
    assert!(parse("export { foo }", &Options::default(), &mut c).is_err());
}

#[test]
fn patterns_and_arrows() {
    let v = json("[a, {b: [c = 1], ...d}] = e; const f = async ({g}, ...h) => g;");
    let assign = &v["body"][0]["expression"];
    assert_eq!(assign["left"]["type"], "ArrayPattern");
    assert_eq!(assign["left"]["elements"][1]["type"], "ObjectPattern");
    assert_eq!(assign["left"]["elements"][1]["properties"][0]["value"]["elements"][0]["type"], "AssignmentPattern");
    assert_eq!(assign["left"]["elements"][1]["properties"][1]["type"], "RestElement");
    let arrow = &v["body"][1]["declarations"][0]["init"];
    assert_eq!(arrow["type"], "ArrowFunctionExpression");
    assert_eq!(arrow["async"], true);
    assert_eq!(arrow["params"][1]["type"], "RestElement");
    assert_eq!(err("({a = 1})"), "Shorthand property assignments are valid only in destructuring patterns (1:4)");
    let v = json("({a = 1} = b)");
    assert_eq!(v["body"][0]["expression"]["left"]["properties"][0]["value"]["type"], "AssignmentPattern");
}

#[test]
fn try_blocks_have_type() {
    let v = json("try {} catch (e) {} finally {}");
    assert_eq!(v["body"][0]["block"]["type"], "BlockStatement");
    assert_eq!(v["body"][0]["handler"]["body"]["type"], "BlockStatement");
    assert_eq!(v["body"][0]["finalizer"]["type"], "BlockStatement");
}

#[test]
fn literals() {
    let v = json("0x10n; 1_000; 'a\\u{1F600}\\uD83D\\uDE00'; `x${1}\\n`");
    assert_eq!(v["body"][0]["expression"]["bigint"], "16");
    assert_eq!(v["body"][1]["expression"]["value"], 1000.0);
    assert_eq!(v["body"][2]["expression"]["value"], "a\u{1F600}\u{1F600}");
    let tpl = &v["body"][3]["expression"];
    assert_eq!(tpl["quasis"][1]["value"]["cooked"], "\n");
    assert_eq!(tpl["quasis"][1]["value"]["raw"], "\\n");
}

#[test]
fn identifiers() {
    assert!(is_identifier_start('a'));
    assert!(is_identifier_start('ü'));
    assert!(!is_identifier_start('1'));
    assert!(is_identifier_char('1'));
    assert!(is_identifier_char('\u{200c}'));
    assert!(is_identifier_start('\u{10400}'));
}

#[test]
fn deep_nesting_is_an_error_not_a_crash() {
    // MAX_DEPTH needs ~16 MiB of stack in debug builds (see expression.rs).
    std::thread::Builder::new().stack_size(64 << 20).spawn(deep_nesting).unwrap().join().unwrap();
}

fn deep_nesting() {
    for (open, close) in [("(", ")"), ("[", "]"), ("{a:", "}"), ("!", ""), ("-", ""), ("a=", ""), ("()=>", "")] {
        let src = format!("x = {}1{}", open.repeat(5000), close.repeat(5000));
        let e = module(&src).unwrap_err();
        assert!(e.message.starts_with("Not enough stack space to parse input"), "{open}: {}", e.message);
    }
    let src = format!("{}x{}", "{".repeat(5000), "}".repeat(5000));
    assert!(module(&src).unwrap_err().message.starts_with("Not enough stack space"));
    let src = format!("{}x", "if (a) ".repeat(5000));
    assert!(module(&src).unwrap_err().message.starts_with("Not enough stack space"));
    let src = format!("let {}a{} = 1", "[".repeat(5000), "]".repeat(5000));
    assert!(module(&src).unwrap_err().message.starts_with("Not enough stack space"));
    // Long flat chains are fine.
    let src = format!("x = 1{}", " + 1".repeat(20000));
    assert!(module(&src).is_ok());
}
