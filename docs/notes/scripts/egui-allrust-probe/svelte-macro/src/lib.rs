//! Probe: `include_svelte!("ui/App.svelte")` runs svelters-compiler at macro expansion.
//! Errors become `compile_error!` at the call site (a proc macro cannot point a span into a
//! non-Rust file). The expansion includes `include_str!` of the file so rustc tracks it.
//! `svelte!(r#"..."#)` checks whether an inline template's diagnostic can be placed at the exact
//! byte inside the string literal on stable (`Literal::subspan`).
use proc_macro::{Literal, TokenStream, TokenTree};
use svelters_compiler::compile::{CompileOptions, compile};
use svelters_compiler::diag::Severity;
use svelters_compiler::semantics::SpecBackend;

fn string_arg(input: TokenStream) -> (Literal, String) {
    let tt = input.into_iter().next().expect("one string literal");
    let TokenTree::Literal(lit) = tt else { panic!("expected a string literal") };
    let s = lit.to_string();
    let v = if let Some(r) = s.strip_prefix("r#\"") { r.strip_suffix("\"#").unwrap().to_string() } else { s[1..s.len() - 1].to_string() };
    (lit, v)
}

fn errors_to_tokens(diags: &[svelters_compiler::diag::Diagnostic], name: &str) -> Option<String> {
    let errs: Vec<String> = diags.iter().filter(|d| d.severity == Severity::Error).map(|d| d.render(name)).collect();
    if errs.is_empty() { None } else { Some(format!("compile_error!({:?})", errs.join("\n"))) }
}

#[proc_macro]
pub fn include_svelte(input: TokenStream) -> TokenStream {
    let (_lit, rel) = string_arg(input);
    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = format!("{root}/{rel}");
    let src = std::fs::read_to_string(&path).unwrap();
    let backend = SpecBackend::builtin("gtk").unwrap();
    let r = compile(&src, &CompileOptions::new(&rel, &backend));
    if let Some(e) = errors_to_tokens(&r.diagnostics, &path) {
        return e.parse().unwrap();
    }
    format!("{{ const _TRACK: &str = include_str!({path:?}); {:?} }}", r.js.unwrap()).parse().unwrap()
}

#[proc_macro]
pub fn svelte(input: TokenStream) -> TokenStream {
    let (lit, src) = string_arg(input);
    let backend = SpecBackend::builtin("gtk").unwrap();
    let r = compile(&src, &CompileOptions::new("inline.svelte", &backend));
    let first = r.diagnostics.iter().find(|d| d.severity == Severity::Error);
    if let Some(d) = first {
        // Raw string: content starts 3 bytes in (`r#"`).
        // The error lands on the whole literal; message carries line:col inside it.
        let span = lit.span(); // Literal::subspan is unstable (proc_macro_span, rust#54725) on 1.97
        let msg = format!("inline.svelte:{}:{}: {}: {}", d.line, d.column, d.code, d.message);
        let mut ts: TokenStream = format!("compile_error!({msg:?})").parse().unwrap();
        ts = ts.into_iter().map(|mut t| { t.set_span(span); t }).collect::<TokenStream>();
        return ts;
    }
    format!("{:?}", r.js.unwrap()).parse().unwrap()
}
