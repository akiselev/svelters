//! Maps QuickJS positions (`module.js:line:col`) in error messages, stacks and console text back
//! to the original `.svelte`/`.ts` sources, using the `<module>.map` files Vite writes next to the
//! bundle. All mapping happens here in Rust; the JS side only produces ordinary stacks.

use crate::engine::JsError;
use ::sourcemap::SourceMap;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

#[derive(Default)]
pub(crate) struct SourceMaps {
    root: Option<PathBuf>,
    /// Module name (slash-separated, relative to `root`) -> its parsed map, or None when there is
    /// no usable map.
    maps: HashMap<String, Option<SourceMap>>,
}

impl SourceMaps {
    /// Starts a new bundle: forgets every cached map and eagerly loads the entry's map, so a
    /// rebuild that lands later cannot change what an already-running bundle maps to.
    pub(crate) fn set_entry(&mut self, entry: &Path) {
        self.maps.clear();
        self.root = entry.parent().map(Path::to_path_buf);
        if let Some(name) = entry.file_name().and_then(|n| n.to_str()) {
            self.map_for(name);
        }
    }

    fn map_for(&mut self, module: &str) -> Option<&SourceMap> {
        let root = self.root.as_ref()?;
        self.maps
            .entry(module.to_string())
            .or_insert_with(|| {
                let path = root.join(format!("{module}.map"));
                let bytes = std::fs::read(path).ok()?;
                SourceMap::from_slice(&bytes).ok()
            })
            .as_ref()
    }

    /// Rewrites every `module.js:line:col` in `text` that the maps can resolve to
    /// `original/path:line:col`. Everything else is left untouched.
    pub(crate) fn map_text(&mut self, text: &str) -> String {
        if self.root.is_none() {
            return text.to_string();
        }
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(pos) = find_position(rest) {
            let Pos { start, end, module, line, col } = pos;
            out.push_str(&rest[..start]);
            let original = &rest[start..end];
            match self.resolve(module, line, col) {
                Some(mapped) => out.push_str(&mapped),
                None => out.push_str(original),
            }
            rest = &rest[end..];
        }
        out.push_str(rest);
        out
    }

    fn resolve(&mut self, module: &str, line: u32, col: u32) -> Option<String> {
        let sm = self.map_for(module)?;
        // QuickJS positions are 1-based; the map's are 0-based.
        let tok = sm.lookup_token(line.checked_sub(1)?, col.saturating_sub(1))?;
        let src = tok.get_source()?;
        Some(format!(
            "{}:{}:{}",
            display_path(src),
            tok.get_src_line() + 1,
            tok.get_src_col() + 1
        ))
    }

    pub(crate) fn map_error(&mut self, mut e: JsError) -> JsError {
        e.message = self.map_text(&e.message);
        e.stack = e.stack.map(|s| self.map_text(&s));
        e
    }
}

struct Pos<'a> {
    start: usize,
    end: usize,
    module: &'a str,
    line: u32,
    col: u32,
}

fn is_path_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | '@' | '+' | '~')
}

/// Finds the next `<name>.js:<line>:<col>` (or `.mjs`) in `s`.
fn find_position(s: &str) -> Option<Pos<'_>> {
    let mut from = 0;
    loop {
        let i = match (s[from..].find(".js:"), s[from..].find(".mjs:")) {
            (Some(a), Some(b)) => from + a.min(b),
            (Some(a), None) | (None, Some(a)) => from + a,
            (None, None) => return None,
        };
        let ext_len = if s[i..].starts_with(".mjs:") { 4 } else { 3 };
        let name_end = i + ext_len;
        let start = s[..i]
            .char_indices()
            .rev()
            .take_while(|(_, c)| is_path_char(*c))
            .last()
            .map_or(i, |(k, _)| k);
        let tail = &s[name_end + 1..];
        let (line, n1) = number(tail);
        if n1 > 0 {
            if let Some(after) = tail[n1..].strip_prefix(':') {
                let (col, n2) = number(after);
                if n2 > 0 {
                    return Some(Pos {
                        start,
                        end: name_end + 1 + n1 + 1 + n2,
                        module: &s[start..name_end],
                        line,
                        col,
                    });
                }
            }
        }
        from = name_end;
    }
}

fn number(s: &str) -> (u32, usize) {
    let n = s.bytes().take_while(u8::is_ascii_digit).count();
    (s[..n].parse().unwrap_or(0), n)
}

/// Source paths in a Vite map are relative to the map's directory (`../src/App.svelte`). Shows
/// them lexically normalised with the leading `..` components dropped (`src/App.svelte`).
fn display_path(src: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for c in Path::new(src.strip_prefix("file://").unwrap_or(src)).components() {
        match c {
            Component::Normal(p) => parts.push(p.to_string_lossy().into_owned()),
            Component::ParentDir => {
                if parts.last().is_some_and(|l| l != "..") {
                    parts.pop();
                } else {
                    parts.push("..".into());
                }
            }
            _ => {}
        }
    }
    while parts.first().is_some_and(|p| p == "..") {
        parts.remove(0);
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_positions_in_stack_lines() {
        let s = "    at <anonymous> (app.js:12:7)\n    at app.js:3:1";
        let p = find_position(s).unwrap();
        assert_eq!((p.module, p.line, p.col), ("app.js", 12, 7));
        assert_eq!(&s[p.start..p.end], "app.js:12:7");
        let rest = &s[p.end..];
        let q = find_position(rest).unwrap();
        assert_eq!((q.module, q.line, q.col), ("app.js", 3, 1));
    }

    #[test]
    fn ignores_text_without_positions() {
        assert!(find_position("see foo.js: for details").is_none());
        assert!(find_position("x.js:12").is_none());
    }

    #[test]
    fn normalises_source_paths() {
        assert_eq!(display_path("../src/App.svelte"), "src/App.svelte");
        assert_eq!(display_path("../../node_modules/svelte/x.js"), "node_modules/svelte/x.js");
        assert_eq!(display_path("a/../b.ts"), "b.ts");
    }
}
