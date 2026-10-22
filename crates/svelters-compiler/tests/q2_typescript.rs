//! Q2: TypeScript modules for the module loader: `.ts`/`.mts` stripped as is
//! (`strip_typescript_module`) and `.svelte.ts` through `compile_module`, with the rules of
//! `<script lang="ts">` (`remove_typescript_nodes`).

use svelters_compiler::compile::{ModuleOptions, compile_module, strip_typescript_module};

fn strip(src: &str) -> String {
    let res = strip_typescript_module(src, "/app/src/m.ts");
    assert!(!res.has_errors(), "{:?}", res.diagnostics);
    res.js.unwrap()
}

fn strip_err(src: &str) -> String {
    let res = strip_typescript_module(src, "/app/src/m.ts");
    assert!(res.has_errors() && res.js.is_none(), "expected an error for {src:?}");
    res.diagnostics.iter().map(|d| format!("{}: {}", d.code, d.message)).collect::<Vec<_>>().join("\n")
}

#[test]
fn types_are_stripped_and_the_javascript_kept() {
    assert_eq!(strip("export const n: number = 1;\n"), "export const n = 1;\n");
    assert_eq!(
        strip("import type { A } from './a';\nimport { b, type C } from './b.js';\nexport type D = A | C;\nexport interface E { x: D }\nexport function f<T>(x: T, y?: E): T { return (x as T)!; }\n"),
        "\nimport { b } from './b.js';\n\n\nexport function f(x, y) { return (x); }\n",
        "removed declarations leave their lines, so positions stay close to the source"
    );
    assert_eq!(strip("declare global { interface Window { a: 1 } }\nexport abstract class K<T> implements I { private x: T; abstract m(): void; }\n").trim(), "export class K { x;  }");
    // no runes, no runtime import: a `.ts` module is plain JavaScript
    let out = strip("export const s = $state(1);\n");
    assert_eq!(out, "export const s = $state(1);\n");
}

#[test]
fn unsupported_typescript_is_an_error() {
    assert!(strip_err("export enum E { A }\n").contains("typescript_invalid_feature"));
    assert!(strip_err("namespace N { export const a = 1; }\n").contains("typescript_invalid_feature"));
    assert!(!strip_err("export const x: = 1;\n").is_empty());
}

#[test]
fn the_source_map_names_the_ts_file() {
    let res = strip_typescript_module("let a: number = 1;\nexport { a };\n", "/app/src/m.ts");
    let map = res.map.unwrap();
    assert!(map.contains("/app/src/m.ts"), "{map}");
}

#[test]
fn svelte_ts_modules_strip_types_and_compile_runes() {
    let opts = ModuleOptions { filename: "/app/src/state.svelte.ts".into(), dev: false, hmr: false };
    let res = compile_module("type L = { text: string };\nexport const label: L = $state<L>({ text: 'a' });\nexport function bump(n: number): void { label.text += n; }\n", &opts);
    assert!(!res.has_errors(), "{:?}", res.diagnostics);
    let js = res.js.unwrap();
    assert!(js.contains("$.proxy({ text: 'a' })") || js.contains("$.state("), "{js}");
    assert!(!js.contains(": L") && !js.contains("type L") && !js.contains(": number") && !js.contains("<L>"), "{js}");

    // the same module named `.svelte.js` is JavaScript: the types are a syntax error
    let js_opts = ModuleOptions { filename: "/app/src/state.svelte.js".into(), dev: false, hmr: false };
    assert!(compile_module("export const n: number = 1;\n", &js_opts).has_errors());
    let err = compile_module("export enum E { A }\n", &opts);
    assert!(err.diagnostics.iter().any(|d| d.code == "typescript_invalid_feature"), "{:?}", err.diagnostics);
}
