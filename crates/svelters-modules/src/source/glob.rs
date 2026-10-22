//! AU2b: Vite's `import.meta.glob` on the live loader, so an app that lists its pages with it (the
//! gallery) runs from source as it does from a Vite build.
//!
//! The call is replaced in the compiled module by an object literal keyed by the matched paths as
//! written relative to the module (`"./pages/a.svelte"`), sorted:
//! - `import.meta.glob('./pages/*.svelte', { eager: true })`: each value is the module namespace (a
//!   static `import * as` appended to the module; imports hoist, so source maps keep their lines);
//! - `{ eager: true, import: 'default' }`: the named export (`import { default as … }`);
//! - without `eager`: each value is `() => import("./pages/a.svelte")` (with `import`, `.then` picks
//!   the export).
//!
//! Patterns are string literals or an array of them, relative (`./`, `../`); `*` and `?` stay inside
//! one path segment, `**` spans segments, `{a,b}` alternates, and a `!` pattern excludes. Other
//! options (`query`, `as`, `base`) fail with a clear error. The file list is read when the module is
//! compiled: a file added later shows up on the next compile of the importing module.

use std::path::{Path, PathBuf};

const CALL: &str = "import.meta.glob";

/// Expands every `import.meta.glob(...)` in `js`, a module at `file`. `Ok(None)`: none in it.
pub(crate) fn expand(js: &str, file: &Path) -> Result<Option<String>, String> {
    if !js.contains(CALL) {
        return Ok(None);
    }
    let dir = file.parent().unwrap_or(Path::new("/"));
    let (mut out, mut imports, mut rest, mut n) = (String::new(), String::new(), js, 0usize);
    while let Some(i) = rest.find(CALL) {
        out.push_str(&rest[..i]);
        let after = &rest[i + CALL.len()..];
        let Some(args) = after.trim_start().strip_prefix('(') else {
            // `import.meta.globEager` or a mention: leave it.
            out.push_str(CALL);
            rest = after;
            continue;
        };
        let mut p = Parser { s: args, at: 0 };
        let call = p.call().map_err(|e| format!("{}: import.meta.glob: {e}", file.display()))?;
        rest = &args[p.at..];
        let files = matches(dir, &call.patterns)?;
        let mut obj = String::from("{");
        for (k, (key, _)) in files.iter().enumerate() {
            let spec = json(key);
            let value = if call.eager {
                let local = format!("__svelters_glob_{n}_{k}");
                match &call.import {
                    Some(name) => imports.push_str(&format!("\nimport {{ {} as {local} }} from {spec};", ident_or_string(name))),
                    None => imports.push_str(&format!("\nimport * as {local} from {spec};")),
                }
                local
            } else {
                match &call.import {
                    Some(name) => format!("() => import({spec}).then((m) => m[{}])", json(name)),
                    None => format!("() => import({spec})"),
                }
            };
            obj.push_str(&format!("{}{spec}: {value}", if k == 0 { "" } else { ", " }));
        }
        obj.push('}');
        out.push_str(&obj);
        n += 1;
    }
    out.push_str(rest);
    out.push_str(&imports);
    if !imports.is_empty() {
        out.push('\n');
    }
    Ok(Some(out))
}

fn json(s: &str) -> String {
    serde_json::Value::String(s.to_string()).to_string()
}

fn ident_or_string(name: &str) -> String {
    let ident = !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$') && !name.starts_with(|c: char| c.is_ascii_digit());
    if ident { name.to_string() } else { json(name) }
}

struct Call {
    patterns: Vec<String>,
    eager: bool,
    import: Option<String>,
}

/// A small parser for the call's literal arguments.
struct Parser<'a> {
    s: &'a str,
    at: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        loop {
            let r = &self.s[self.at..];
            let t = r.trim_start();
            self.at += r.len() - t.len();
            if t.starts_with("//") {
                self.at += t.find('\n').unwrap_or(t.len());
            } else if t.starts_with("/*") {
                self.at += t.find("*/").map_or(t.len(), |j| j + 2);
            } else {
                return;
            }
        }
    }
    fn eat(&mut self, c: char) -> bool {
        self.ws();
        if self.s[self.at..].starts_with(c) {
            self.at += c.len_utf8();
            true
        } else {
            false
        }
    }
    fn string(&mut self) -> Result<String, String> {
        self.ws();
        let r = &self.s[self.at..];
        let q = r.chars().next().filter(|c| matches!(c, '\'' | '"' | '`')).ok_or("want a string literal")?;
        let mut out = String::new();
        let mut chars = r[1..].char_indices();
        while let Some((j, c)) = chars.next() {
            match c {
                '\\' => {
                    if let Some((_, e)) = chars.next() {
                        out.push(match e {
                            'n' => '\n',
                            't' => '\t',
                            e => e,
                        });
                    }
                }
                '$' if q == '`' && r[1 + j..].starts_with("${") => return Err("a template literal with ${…} is not a literal".into()),
                c if c == q => {
                    self.at += 1 + j + 1;
                    return Ok(out);
                }
                c => out.push(c),
            }
        }
        Err("unterminated string".into())
    }
    fn word(&mut self) -> String {
        self.ws();
        let r = &self.s[self.at..];
        let n = r.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(r.len());
        self.at += n;
        r[..n].to_string()
    }
    fn call(&mut self) -> Result<Call, String> {
        let mut patterns = Vec::new();
        if self.eat('[') {
            while !self.eat(']') {
                patterns.push(self.string()?);
                self.eat(',');
            }
        } else {
            patterns.push(self.string()?);
        }
        let (mut eager, mut import) = (false, None);
        if self.eat(',') && self.eat('{') {
            while !self.eat('}') {
                self.ws();
                let key = if self.s[self.at..].starts_with(['\'', '"']) { self.string()? } else { self.word() };
                if key.is_empty() || !self.eat(':') {
                    return Err("want `{ key: literal }` options".into());
                }
                match key.as_str() {
                    "eager" => {
                        let v = self.word();
                        eager = match v.as_str() {
                            "true" => true,
                            "false" => false,
                            _ => return Err("eager wants true or false".into()),
                        };
                    }
                    "import" => import = Some(self.string()?),
                    other => return Err(format!("option `{other}` is not supported on the live loader (eager and import are)")),
                }
                self.eat(',');
            }
        }
        self.eat(',');
        if !self.eat(')') {
            return Err("want `import.meta.glob(patterns, { eager, import })`".into());
        }
        if patterns.iter().all(|p| p.starts_with('!')) {
            return Err("no positive pattern".into());
        }
        Ok(Call { patterns, eager, import })
    }
}

/// The files under `dir` matching `patterns`, keyed as written (`./pages/a.svelte`), sorted.
fn matches(dir: &Path, patterns: &[String]) -> Result<Vec<(String, PathBuf)>, String> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    for pat in patterns.iter().filter(|p| !p.starts_with('!')) {
        if !(pat.starts_with("./") || pat.starts_with("../")) {
            return Err(format!("import.meta.glob({pat:?}): only relative patterns (./, ../) work on the live loader"));
        }
        // The literal directory prefix the walk starts from.
        let segs: Vec<&str> = pat.split('/').collect();
        let fixed = segs.iter().take_while(|s| !s.contains(['*', '?', '{', '['])).count().min(segs.len() - 1);
        let base_rel = segs[..fixed].join("/");
        let base = dir.join(&base_rel);
        let mut files = Vec::new();
        walk(&base, &mut files);
        for f in files {
            let Ok(sub) = f.strip_prefix(&base) else { continue };
            let key = format!("{base_rel}/{}", sub.to_string_lossy().replace('\\', "/"));
            if glob_match(pat, &key) && !patterns.iter().filter_map(|p| p.strip_prefix('!')).any(|neg| glob_match(neg, &key)) && !out.iter().any(|(k, _)| *k == key) {
                out.push((key, f));
            }
        }
    }
    out.sort();
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name();
        if p.is_dir() {
            if name != "node_modules" && !name.to_string_lossy().starts_with('.') {
                walk(&p, out);
            }
        } else {
            out.push(p);
        }
    }
}

/// `*`/`?` within a segment, `**` across segments, `{a,b}` alternation.
fn glob_match(pat: &str, path: &str) -> bool {
    if let Some(open) = pat.find('{')
        && let Some(close) = pat[open..].find('}').map(|j| open + j)
    {
        return pat[open + 1..close].split(',').any(|alt| glob_match(&format!("{}{alt}{}", &pat[..open], &pat[close + 1..]), path));
    }
    fn segs(p: &[&str], s: &[&str]) -> bool {
        match p.first() {
            None => s.is_empty(),
            Some(&"**") => (0..=s.len()).any(|k| segs(&p[1..], &s[k..])),
            Some(seg) => !s.is_empty() && seg_match(seg.as_bytes(), s[0].as_bytes()) && segs(&p[1..], &s[1..]),
        }
    }
    fn seg_match(p: &[u8], s: &[u8]) -> bool {
        match p.first() {
            None => s.is_empty(),
            Some(b'*') => (0..=s.len()).any(|k| seg_match(&p[1..], &s[k..])),
            Some(b'?') => !s.is_empty() && seg_match(&p[1..], &s[1..]),
            Some(c) => s.first() == Some(c) && seg_match(&p[1..], &s[1..]),
        }
    }
    let p: Vec<&str> = pat.split('/').collect();
    let s: Vec<&str> = path.split('/').collect();
    segs(&p, &s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs_match_segments() {
        assert!(glob_match("./pages/*.svelte", "./pages/a.svelte"));
        assert!(!glob_match("./pages/*.svelte", "./pages/sub/a.svelte"));
        assert!(glob_match("./pages/**/*.svelte", "./pages/sub/a.svelte"));
        assert!(glob_match("./pages/**/*.svelte", "./pages/a.svelte"));
        assert!(glob_match("./p/?.{js,ts}", "./p/a.ts"));
        assert!(!glob_match("./p/?.{js,ts}", "./p/ab.ts"));
    }

    #[test]
    fn expands_eager_lazy_and_named() {
        let dir = std::env::temp_dir().join(format!("svelters-glob-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("pages/sub")).unwrap();
        for f in ["pages/b.svelte", "pages/a.svelte", "pages/skip.svelte", "pages/sub/c.svelte", "pages/x.js"] {
            std::fs::write(dir.join(f), "").unwrap();
        }
        let file = dir.join("App.svelte");
        let js = "const m = import.meta.glob(['./pages/*.svelte', '!./pages/skip.svelte'], { eager: true });\nconst l = import.meta.glob(\"./pages/**/*.svelte\", { import: 'default' });\n";
        let out = expand(js, &file).unwrap().unwrap();
        assert!(out.starts_with("const m = {\"./pages/a.svelte\": __svelters_glob_0_0, \"./pages/b.svelte\": __svelters_glob_0_1};\n"), "{out}");
        assert!(out.contains("const l = {\"./pages/a.svelte\": () => import(\"./pages/a.svelte\").then((m) => m[\"default\"]), "), "{out}");
        assert!(out.contains("\"./pages/sub/c.svelte\": () => import("), "{out}");
        assert!(!out.contains("x.js"));
        assert!(out.ends_with("\nimport * as __svelters_glob_0_0 from \"./pages/a.svelte\";\nimport * as __svelters_glob_0_1 from \"./pages/b.svelte\";\n"), "{out}");
        let named = expand("import.meta.glob('./pages/a.svelte', { eager: true, import: 'default' })", &file).unwrap().unwrap();
        assert!(named.contains("import { default as __svelters_glob_0_0 } from \"./pages/a.svelte\";"), "{named}");
        assert!(expand("import.meta.glob('./pages/*', { query: '?raw' })", &file).unwrap_err().contains("query"));
        assert!(expand("import.meta.glob('pages/*')", &file).unwrap_err().contains("relative"));
        assert_eq!(expand("const x = 1;", &file).unwrap(), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
