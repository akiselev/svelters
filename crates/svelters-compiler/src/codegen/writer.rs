//! Output buffer for generated JavaScript with source-map mappings back into the `.svelte` file.
//!
//! Generated code is assembled from two kinds of text: copied/edited source ranges (mapped at
//! every chunk start and line start, see `edit.rs`) and synthesized runtime calls (mapped to the
//! template node they implement, e.g. an element's tag). Mappings use UTF-16 columns, as source
//! maps require.

use crate::diag::LineIndex;

pub struct Writer<'s> {
    out: String,
    /// Generated position of the end of `out`: (line, UTF-16 column).
    line: u32,
    col: u32,
    src_lines: LineIndex<'s>,
    /// (gen line, gen col, src line, src col)
    mappings: Vec<(u32, u32, u32, u32)>,
    indent: usize,
    /// A sink ([`Writer::sink`]): nothing is kept.
    discard: bool,
}

impl<'s> Writer<'s> {
    pub fn new(source: &'s str) -> Writer<'s> {
        Writer { out: String::new(), line: 0, col: 0, src_lines: LineIndex::new(source), mappings: Vec::new(), indent: 0, discard: false }
    }

    /// A writer that keeps nothing: code generation run for its diagnostics only (CX1's check).
    pub fn sink() -> Writer<'static> {
        Writer { out: String::new(), line: 0, col: 0, src_lines: LineIndex::new(""), mappings: Vec::new(), indent: 0, discard: true }
    }

    /// Records that the next output position corresponds to byte `offset` of the source.
    pub fn map(&mut self, offset: u32) {
        if self.discard {
            return;
        }
        let (l, c) = self.src_lines.line_col(offset);
        if self.mappings.last() != Some(&(self.line, self.col, l, c)) {
            self.mappings.push((self.line, self.col, l, c));
        }
    }

    /// Appends text with no mapping of its own.
    pub fn write(&mut self, s: &str) {
        if self.discard {
            return;
        }
        for ch in s.chars() {
            if ch == '\n' {
                self.line += 1;
                self.col = 0;
            } else {
                self.col += ch.len_utf16() as u32;
            }
        }
        self.out.push_str(s);
    }

    /// Appends `s` mapped to `offset`.
    pub fn write_at(&mut self, offset: u32, s: &str) {
        self.map(offset);
        self.write(s);
    }

    /// Appends a copy of `source[start..end]`, mapping every line start to its origin.
    pub fn copy(&mut self, source: &str, start: u32, end: u32) {
        if self.discard {
            return;
        }
        let text = &source[start as usize..end as usize];
        self.map(start);
        let mut off = start;
        for (i, piece) in text.split('\n').enumerate() {
            if i > 0 {
                self.write("\n");
                off += 1;
                if !piece.is_empty() {
                    self.map(off);
                }
            }
            self.write(piece);
            off += piece.len() as u32;
        }
    }

    pub fn indent(&mut self) {
        self.indent += 1;
    }

    pub fn dedent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    /// Starts a new line at the current indentation.
    pub fn newline(&mut self) {
        self.write("\n");
        for _ in 0..self.indent {
            self.write("\t");
        }
    }

    pub fn code(&self) -> &str {
        &self.out
    }

    /// Finishes into (code, source map JSON).
    pub fn finish(self, filename: &str, source: &str, out_file: &str) -> (String, String) {
        let mut b = sourcemap::SourceMapBuilder::new(Some(out_file));
        let src_id = b.add_source(filename);
        b.set_source_contents(src_id, Some(source));
        for &(gl, gc, sl, sc) in &self.mappings {
            b.add_raw(gl, gc, sl, sc, Some(src_id), None, false);
        }
        let mut buf = Vec::new();
        b.into_sourcemap().to_writer(&mut buf).expect("source map serializes");
        (self.out, String::from_utf8(buf).expect("source map is UTF-8"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_copies_and_synthesized_text() {
        let src = "let a = 1;\nlet b = a;\n";
        let mut w = Writer::new(src);
        w.write("// gen\n");
        w.write_at(4, "$.state(");
        w.copy(src, 11, 21);
        w.write(")");
        let (code, map) = w.finish("App.svelte", src, "App.js");
        assert_eq!(code, "// gen\n$.state(let b = a;)");
        let sm = sourcemap::SourceMap::from_slice(map.as_bytes()).unwrap();
        let t = sm.lookup_token(1, 0).unwrap();
        assert_eq!((t.get_src_line(), t.get_src_col()), (0, 4));
        let t = sm.lookup_token(1, 8).unwrap();
        assert_eq!((t.get_src_line(), t.get_src_col()), (1, 0));
    }
}
