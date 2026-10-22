//! TypeScript in the native pipeline (owner: agent TS).
//!
//! Svelte parses `<script lang="ts">` (and the template expressions of such a component) with
//! acorn-typescript and, in `compile()`, removes the TypeScript from the AST
//! (`svelte::parse::remove_typescript_nodes`) before analysis. This compiler emits the user's
//! JavaScript as span edits over the source, so the TypeScript text must also go: [`strip`]
//! runs `remove_typescript_nodes` and returns the source ranges to delete; [`TsStrip::apply`]
//! adds them to an [`Edits`] as empty overwrites (edits strictly inside a removed range, e.g. a
//! rune rewrite inside a removed `declare` statement, are dropped by `Edits` itself).
//!
//! Errors are Svelte's `typescript_invalid_feature` (enums, decorators, accessor fields,
//! parameter properties of constructors, namespaces with values; see the module docs of
//! `remove_typescript_nodes` for the cases this compiler adds).

use crate::codegen::edit::Edits;
use crate::estree::Span;
use crate::svelte::ast::Root;
use crate::svelte::errors::CompileError;
pub use crate::svelte::parse::remove_typescript_nodes::TsStrip;
use crate::svelte::parse::remove_typescript_nodes::{remove_typescript_nodes, remove_typescript_nodes_program};

/// Removes the TypeScript from a parsed `lang="ts"` component (a no-op for JavaScript ones) and
/// returns the source ranges that held it.
pub fn strip(root: &mut Root, src: &str) -> Result<TsStrip, CompileError> {
    let mut ranges = TsStrip::default();
    if root.ts {
        remove_typescript_nodes(root, src, Some(&mut ranges))?;
        ranges.ranges.sort_unstable();
        ranges.ranges.dedup();
    }
    Ok(ranges)
}

/// Removes the TypeScript from a module parsed with TypeScript on (`.ts`, `.svelte.ts`) and returns
/// the source ranges that held it.
pub fn strip_program(program: &mut crate::estree::Program, src: &str) -> Result<TsStrip, CompileError> {
    let mut ranges = TsStrip::default();
    remove_typescript_nodes_program(program, src, Some(&mut ranges))?;
    ranges.ranges.sort_unstable();
    ranges.ranges.dedup();
    Ok(ranges)
}

/// Whether a module file is TypeScript, by its name: `.ts`, `.mts` (so `.svelte.ts` too).
pub fn is_ts_module(filename: &str) -> bool {
    filename.ends_with(".ts") || filename.ends_with(".mts")
}

impl TsStrip {
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// Adds the removals inside `span` (all of them when `None`) to `edits`.
    pub fn apply(&self, edits: &mut Edits, span: Option<Span>) {
        for &(start, end) in &self.ranges {
            if span.is_none_or(|s| start >= s.start && end <= s.end) {
                edits.overwrite(start, end, "");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svelte::parse::{ParseOptions, parse};

    /// Parses `src`, strips it and applies the ranges to the instance script's text.
    fn script(src: &str) -> String {
        let mut root = parse(src, &ParseOptions { loose: false, native: true }).expect("parse");
        let strip = strip(&mut root, src).expect("strip");
        let s = root.instance.as_ref().expect("instance script");
        let (a, z) = (s.content.start, s.content.end);
        let mut edits = Edits::new();
        strip.apply(&mut edits, None);
        edits.apply(src, a, z).trim().to_string()
    }

    fn err(src: &str) -> String {
        let mut root = parse(src, &ParseOptions { loose: false, native: true }).expect("parse");
        strip(&mut root, src).expect_err("typescript_invalid_feature").message
    }

    fn ts(body: &str) -> String {
        script(&format!("<script lang=\"ts\">{body}</script>"))
    }

    #[test]
    fn annotations_and_generics() {
        assert_eq!(ts("let x: number = 1;"), "let x = 1;");
        assert_eq!(ts("let y!: string;"), "let y;");
        assert_eq!(ts("function f<T>(a: T, b?: number): T { return a; }"), "function f(a, b) { return a; }");
        assert_eq!(ts("const g = <T,>(a: T): T => a;"), "const g = (a) => a;");
        assert_eq!(ts("const h = async (e: MouseEvent): Promise<void> => {};"), "const h = async (e) => {};");
        assert_eq!(ts("let { a, b }: Props = $props();"), "let { a, b } = $props();");
        assert_eq!(ts("let s = $state<number[]>([]);"), "let s = $state([]);");
        assert_eq!(ts("try {} catch (e: unknown) {}"), "try {} catch (e) {}");
        assert_eq!(ts("function k(this: Window, a: string) {}"), "function k(a) {}");
    }

    #[test]
    fn expressions() {
        assert_eq!(ts("let a = b as number;"), "let a = b;");
        assert_eq!(ts("let a = (b as any).c;"), "let a = (b).c;");
        assert_eq!(ts("let a = b satisfies T;"), "let a = b;");
        assert_eq!(ts("let a = b!.c!;"), "let a = b.c;");
        assert_eq!(ts("let a = <number>b;"), "let a = b;");
        assert_eq!(ts("let a = f<string>;"), "let a = f;");
        assert_eq!(ts("(x as any) = 1;"), "(x) = 1;");
        assert_eq!(ts("let o = { m<T>(x: T) { return x; } };"), "let o = { m(x) { return x; } };");
        assert_eq!(ts("let a = new Map<string, number>();"), "let a = new Map();");
    }

    #[test]
    fn declarations() {
        assert_eq!(ts("interface A { a: number }\ntype B = string;\nlet x = 1;"), "\nlet x = 1;".trim());
        assert_eq!(ts("import type { A } from './a';\nimport { type B, C } from './b';"), "import { C } from './b';");
        assert_eq!(ts("import { C, type B } from './b';"), "import { C } from './b';");
        assert_eq!(ts("declare const x: number;\ndeclare function f(): void;\nlet y;"), "let y;");
        assert_eq!(ts("export type { A };\nlet z;"), "let z;");
        assert_eq!(ts("namespace N { export type T = number; }\nlet z;"), "let z;");
    }

    #[test]
    fn classes() {
        assert_eq!(
            ts("abstract class A<T> extends B<T> implements C, D {\n private x: number = 1;\n declare y: string;\n abstract m(): void;\n readonly z?: T;\n n(a: T): void;\n n(a: T) {}\n}"),
            "class A extends B {\n x = 1;\n \n \n z;\n \n n(a) {}\n}"
        );
    }

    #[test]
    fn invalid_features() {
        assert!(err("<script lang=\"ts\">enum E { A }</script>").contains("like enums are not natively supported"));
        assert!(err("<script lang=\"ts\">class A { constructor(private x: number) {} }</script>").contains("accessibility modifiers on constructor parameters"));
        assert!(err("<script lang=\"ts\">namespace N { export const x = 1; }</script>").contains("namespaces with non-type nodes"));
        assert!(err("<script lang=\"ts\">class A { accessor x = 1; }</script>").contains("accessor fields"));
    }

    #[test]
    fn javascript_is_untouched() {
        let src = "<script>let x = 1;</script>";
        let mut root = parse(src, &ParseOptions { loose: false, native: true }).unwrap();
        assert!(strip(&mut root, src).unwrap().is_empty());
    }
}
