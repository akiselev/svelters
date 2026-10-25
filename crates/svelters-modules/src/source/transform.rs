//! Turning one source file into an ES module, by kind.

use std::path::Path;
use svelters_compiler::compile::{CompileOptions, CompileResult, ModuleOptions, compile, compile_module, strip_typescript_module};
use svelters_compiler::diag::Severity;
use svelters_compiler::scoped_css::ScopedCss;
use svelters_compiler::semantics::SpecBackend;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// `.svelte`: `compile`.
    Component,
    /// `.svelte.js` / `.svelte.ts`: `compile_module` (runes outside components; TypeScript stripped).
    RunesModule,
    /// `.ts` / `.mts`: the types stripped (`strip_typescript_module`).
    Ts,
    /// `.js` / `.mjs`: as is.
    Js,
    /// `.json`: `export default <the JSON>`.
    Json,
    /// Anything else: an asset, served by the loader as a generated module (AS1, `asset_module` in
    /// `mod.rs`), never transformed here.
    Asset,
    /// A kind this loader does not load yet; the message says why.
    Unsupported(&'static str),
}

impl Kind {
    pub(crate) fn of(path: &Path) -> Kind {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.ends_with(".svelte") {
            Kind::Component
        } else if name.ends_with(".svelte.js") || name.ends_with(".svelte.ts") {
            Kind::RunesModule
        } else if name.ends_with(".d.ts") || name.ends_with(".d.mts") {
            Kind::Unsupported("a declaration file has no runtime code: import the module it describes, or use `import type`")
        } else if name.ends_with(".ts") || name.ends_with(".mts") {
            Kind::Ts
        } else if name.ends_with(".tsx") || name.ends_with(".jsx") {
            Kind::Unsupported("JSX is not supported by the module loader: write the UI as .svelte components")
        } else if name.ends_with(".cjs") || name.ends_with(".cts") {
            Kind::Unsupported("CommonJS modules are not supported by the module loader (QuickJS has no `require`): use the package's ES module build")
        } else if name.ends_with(".css") {
            Kind::Unsupported("CSS imports are not supported by the module loader: put styles in a component's <style>")
        } else if name.ends_with(".js") || name.ends_with(".mjs") {
            Kind::Js
        } else if name.ends_with(".json") {
            Kind::Json
        } else {
            Kind::Asset
        }
    }
}

/// The compiler settings shared by every component of an app.
pub(crate) struct Compiler {
    pub(crate) sem: SpecBackend,
    pub(crate) dev: bool,
    /// Components get the `--hmr` wrapper (needs `dev`).
    pub(crate) hmr: bool,
    /// The app root: a component's scope class is derived from its logical id (Q4b).
    pub(crate) root: std::path::PathBuf,
}

/// A transformed module: its code, map and warnings, or the error that stops the load.
#[derive(Debug)]
pub(crate) struct Output {
    pub(crate) js: String,
    pub(crate) map: Option<String>,
    pub(crate) warnings: Vec<String>,
    /// LD2: a component's scoped `<style>` (Q3w), for the backend's style engine (egui). `None`
    /// without one, when it is empty or `css="injected"`, and on the other backends.
    pub(crate) css: Option<ScopedCss>,
}

impl Compiler {
    /// LD2: this backend takes a component's scoped `<style>` as text (`registerStyles(id, { css })`,
    /// installed by its style engine): its spec says the backend runs the CSS engine (`style.engine`
    /// `backend`: egui, skelter). The other backends get their sheets from the Vite build's JS
    /// translator (`@svelters/style/build`), which the live loader does not run.
    pub(crate) fn styles(&self) -> bool {
        self.sem.spec().style.engine == "backend"
    }

    /// Compiles a component or a runes module (without the error check): the raw result, for
    /// diagnostics.
    pub(crate) fn compile(&self, kind: Kind, path: &Path, src: &str) -> Option<CompileResult> {
        let filename = path.to_string_lossy().into_owned();
        match kind {
            Kind::Component => {
                let mut opts = CompileOptions::new(filename, &self.sem);
                opts.dev = self.dev;
                opts.hmr = self.hmr;
                opts.scoped_css = self.styles();
                if opts.scoped_css {
                    // Q4b: the scope class (`svelte-<hash>`) of the logical id (`app/src/App.svelte`),
                    // not of the build machine's path, so it is the same on every machine.
                    let id = super::build::logical_id(&self.root, path);
                    opts.css_scope = Some(format!("svelte-{}", svelters_compiler::svelte::css::hash(&id)));
                }
                Some(compile(src, &opts))
            }
            Kind::RunesModule => Some(compile_module(src, &ModuleOptions { filename, dev: self.dev, hmr: self.hmr })),
            Kind::Ts => Some(strip_typescript_module(src, &filename)),
            _ => None,
        }
    }

    pub(crate) fn transform(&self, path: &Path, src: String) -> Result<Output, String> {
        let shown = path.display().to_string();
        let kind = Kind::of(path);
        match kind {
            Kind::Component | Kind::RunesModule | Kind::Ts => {
                let res = self.compile(kind, path, &src).expect("a compiled kind");
                let rendered = |sev: Severity| res.diagnostics.iter().filter(|d| d.severity == sev).map(|d| d.render(&shown)).collect::<Vec<_>>();
                if res.has_errors() || res.js.is_none() {
                    let errors = rendered(Severity::Error);
                    return Err(if errors.is_empty() { format!("{shown}: the compiler produced no output") } else { errors.join("\n") });
                }
                let mut warnings = rendered(Severity::Warning);
                if kind == Kind::Component && !self.styles() && has_style(&src) {
                    warnings.push(format!(
                        "{shown}: <style> is applied from source only on backends that run the CSS engine (egui, skelter; {} sheets are translated by the Vite build); build with Vite to get its styles",
                        self.sem.spec().backend
                    ));
                }
                let css = res.css.filter(|c| !c.injected && !c.code.trim().is_empty());
                Ok(Output { js: res.js.unwrap_or_default(), map: res.map, warnings, css })
            }
            Kind::Js => Ok(Output { js: src, map: None, warnings: Vec::new(), css: None }),
            Kind::Json => {
                serde_json::from_str::<serde_json::Value>(&src).map_err(|e| format!("{shown}: invalid JSON: {e}"))?;
                Ok(Output { js: format!("export default {};\n", src.trim()), map: None, warnings: Vec::new(), css: None })
            }
            // `SourceProvider::check` of an asset file: nothing to compile.
            Kind::Asset => Ok(Output { js: String::new(), map: None, warnings: Vec::new(), css: None }),
            Kind::Unsupported(why) => Err(format!("{shown}: {why}")),
        }
    }
}

/// The component has a `<style>` with something in it.
fn has_style(src: &str) -> bool {
    let mut rest = src;
    while let Some(i) = rest.find("<style") {
        let after = &rest[i + 6..];
        if after.starts_with(['>', ' ', '\t', '\n', '\r']) {
            let body = after.find('>').map_or("", |j| &after[j + 1..]);
            let end = body.find("</style>").unwrap_or(body.len());
            return !body[..end].trim().is_empty();
        }
        rest = after;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_by_extension() {
        assert_eq!(Kind::of(Path::new("/a/App.svelte")), Kind::Component);
        assert_eq!(Kind::of(Path::new("/a/state.svelte.js")), Kind::RunesModule);
        assert_eq!(Kind::of(Path::new("/a/x.js")), Kind::Js);
        assert_eq!(Kind::of(Path::new("/a/x.mjs")), Kind::Js);
        assert_eq!(Kind::of(Path::new("/a/data.json")), Kind::Json);
        assert_eq!(Kind::of(Path::new("/a/icon.png")), Kind::Asset);
        assert_eq!(Kind::of(Path::new("/a/x.ts")), Kind::Ts);
        assert_eq!(Kind::of(Path::new("/a/x.mts")), Kind::Ts);
        assert_eq!(Kind::of(Path::new("/a/x.svelte.ts")), Kind::RunesModule);
        assert!(matches!(Kind::of(Path::new("/a/x.d.ts")), Kind::Unsupported(_)));
        assert!(matches!(Kind::of(Path::new("/a/x.tsx")), Kind::Unsupported(_)));
        assert!(matches!(Kind::of(Path::new("/a/x.cts")), Kind::Unsupported(_)));
    }

    #[test]
    fn finds_non_empty_styles() {
        assert!(has_style("<window/>\n<style>\n label { color: red; }\n</style>"));
        assert!(has_style("<style lang=\"css\">a{}</style>"));
        assert!(!has_style("<window/>\n<style>\n</style>"));
        assert!(!has_style("<window/><styles-panel/>"));
    }

    #[test]
    fn json_and_assets() {
        let c = Compiler { sem: SpecBackend::builtin("gtk").unwrap(), dev: false, hmr: false, root: "/app".into() };
        let out = c.transform(Path::new("/a/d.json"), "{\"n\": 1}\n".into()).unwrap();
        assert_eq!(out.js, "export default {\"n\": 1};\n");
        assert!(c.transform(Path::new("/a/d.json"), "{n: 1}".into()).unwrap_err().contains("invalid JSON"));
        assert_eq!(c.transform(Path::new("/a/i.png"), String::new()).unwrap().js, "");
    }
}
