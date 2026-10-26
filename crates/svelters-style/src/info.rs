//! DZ0: what a designer's or inspector's "matched rules" pane shows for a node: plain data, shared by
//! every backend's style engine (svelters-egui's scene, skelter-core's document) and
//! svelters-automation's `style.matched`.

/// DZ0: a place in a component's `<style>`: the component's `.svelte` path (as the compiler got
/// it) and a byte offset from the start of the `<style>` element's content. Relative to the style
/// content so it stays right while the template above it is edited: add the content start of the
/// file as it is now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleSource {
    pub file: String,
    pub style_offset: u32,
}

/// DZ0: one declaration of a [`MatchedRuleInfo`].
#[derive(Clone, Debug, PartialEq)]
pub struct DeclInfo {
    pub name: String,
    pub value: String,
    pub important: bool,
    /// Where it is in the component's `<style>` (dev builds; `None` for theme and host sheets).
    pub source: Option<StyleSource>,
    /// DZ0b2: it wins the cascade for its property (for a shorthand, for at least one longhand)
    /// over every other rule and the node's `style=` and forced declarations. A prop the runtime
    /// set still wins over it (CSS.md §2.8).
    pub winner: bool,
}

/// DZ0: a rule that matches a node, as a designer's or inspector's "matched rules" pane shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchedRuleInfo {
    /// The sheet's name (a component's logical id, `@theme…`, a host sheet); `None` for a
    /// generated utility rule.
    pub sheet: Option<String>,
    /// The selectors as CSS (after nesting; the scoping class included).
    pub selectors: String,
    /// The highest specificity among the selectors that matched.
    pub specificity: u32,
    /// The rule's position in the sheet text (1-based line, UTF-16 column).
    pub line: u32,
    pub column: u32,
    /// Where the rule is in the component's `<style>` (dev builds).
    pub source: Option<StyleSource>,
    pub decls: Vec<DeclInfo>,
}

/// DZ0 (dev): where a component sheet's text came from: the component's `.svelte` path (as the
/// compiler got it), the runs of the sheet text that are untouched `<style>` text, and the sheet's
/// line starts (rules and declarations carry a line and a UTF-16 column).
#[derive(Debug)]
pub struct SheetSource {
    file: String,
    map: Vec<(u32, u32, u32)>,
    css: String,
    lines: Vec<u32>,
}

impl SheetSource {
    /// `None` when `map` (`out,src,len;…`) doesn't parse.
    pub fn new(file: &str, map: &str, css: &str) -> Option<SheetSource> {
        let map = parse_map(map)?;
        let lines = std::iter::once(0).chain(css.match_indices('\n').map(|(i, _)| i as u32 + 1)).collect();
        Some(SheetSource { file: file.to_string(), map, css: css.to_string(), lines })
    }

    /// The `<style>`-relative offset of the sheet text at `line` (1-based) and `column` (1-based,
    /// UTF-16 units, as cssparser counts), if that text is untouched style text.
    pub fn at(&self, line: u32, column: u32) -> Option<u32> {
        let start = *self.lines.get(line.checked_sub(1)? as usize)? as usize;
        let text = self.css.get(start..)?;
        let mut units = 1;
        let mut byte = start;
        for c in text.chars() {
            if units >= column || c == '\n' {
                break;
            }
            units += c.len_utf16() as u32;
            byte += c.len_utf8();
        }
        let at = byte as u32;
        let k = self.map.partition_point(|&(o, _, _)| o <= at).checked_sub(1)?;
        let (o, s, l) = self.map[k];
        (at < o + l).then_some(s + (at - o))
    }
}

/// `out,src,len;…` runs (svelters-compiler `ScopedCss::source_map_text`; parsed here, as the
/// backends do not depend on the compiler).
fn parse_map(text: &str) -> Option<Vec<(u32, u32, u32)>> {
    if text.is_empty() {
        return Some(Vec::new());
    }
    text.split(';')
        .map(|run| {
            let mut it = run.split(',').map(|x| x.parse::<u32>().ok());
            Some((it.next()??, it.next()??, it.next()??))
        })
        .collect()
}

impl SheetSource {
    /// The component's `.svelte` path.
    pub fn file(&self) -> &str {
        &self.file
    }

    /// The [`StyleSource`] of the sheet text at `line` and `column` (a rule's or a declaration's),
    /// if that text is untouched style text.
    pub fn source_at(&self, line: u32, column: u32) -> Option<StyleSource> {
        Some(StyleSource { file: self.file.clone(), style_offset: self.at(line, column)? })
    }
}
