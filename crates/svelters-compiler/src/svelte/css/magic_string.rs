//! The subset of `magic-string` 0.30.21 the CSS transform uses (`appendLeft`, `appendRight`,
//! `prependRight`, `overwrite`, `update`, `remove`, `toString`), with its chunk semantics:
//! inserts attach to the chunk ending (`*Left`) or starting (`*Right`) at the index, `remove` and
//! `overwrite` drop the attached inserts, `update` keeps those of its first chunk.
//!
//! SVELTERS: no source maps, no `move`; indices are byte offsets. Splitting an edited chunk
//! (an error upstream) panics.

use std::collections::BTreeMap;

struct Chunk {
    end: usize,
    intro: String,
    outro: String,
    /// `None`: the original text
    content: Option<String>,
    edited: bool,
}

pub struct MagicString<'s> {
    original: &'s str,
    intro: String,
    outro: String,
    /// chunks by start
    chunks: BTreeMap<usize, Chunk>,
}

impl<'s> MagicString<'s> {
    pub fn new(original: &'s str) -> MagicString<'s> {
        let mut chunks = BTreeMap::new();
        chunks.insert(0, Chunk { end: original.len(), intro: String::new(), outro: String::new(), content: None, edited: false });
        MagicString { original, intro: String::new(), outro: String::new(), chunks }
    }

    pub fn original(&self) -> &'s str {
        self.original
    }

    fn split(&mut self, index: usize) {
        if index == 0 || index >= self.original.len() || self.chunks.contains_key(&index) {
            return;
        }
        let (&start, chunk) = self.chunks.range_mut(..index).next_back().expect("chunk before index");
        assert!(!(chunk.edited && chunk.content.as_ref().is_some_and(|c| !c.is_empty())), "Cannot split a chunk that has already been edited");
        let end = chunk.end;
        let mut new = Chunk { end, intro: String::new(), outro: std::mem::take(&mut chunk.outro), content: None, edited: false };
        chunk.end = index;
        if chunk.edited {
            new.content = Some(String::new());
            new.edited = true;
            new.outro.clear();
            new.intro.clear();
            chunk.content = Some(String::new());
        }
        let _ = start;
        self.chunks.insert(index, new);
    }

    /// `byEnd[index]`
    fn by_end(&mut self, index: usize) -> Option<&mut Chunk> {
        if index == 0 {
            return None;
        }
        let (_, c) = self.chunks.range_mut(..index).next_back()?;
        (c.end == index).then_some(c)
    }

    /// `byStart[index]`
    fn by_start(&mut self, index: usize) -> Option<&mut Chunk> {
        self.chunks.get_mut(&index)
    }

    pub fn append_left(&mut self, index: usize, content: &str) {
        self.split(index);
        match self.by_end(index) {
            Some(c) => c.outro.push_str(content),
            None => self.intro.push_str(content),
        }
    }

    pub fn append_right(&mut self, index: usize, content: &str) {
        self.split(index);
        match self.by_start(index) {
            Some(c) => c.intro.push_str(content),
            None => self.outro.push_str(content),
        }
    }

    pub fn prepend_right(&mut self, index: usize, content: &str) {
        self.split(index);
        match self.by_start(index) {
            Some(c) => c.intro.insert_str(0, content),
            None => self.outro.insert_str(0, content),
        }
    }

    /// `overwrite(start, end, content)`
    pub fn overwrite(&mut self, start: usize, end: usize, content: &str) {
        self.update_impl(start, end, content, true);
    }

    /// `update(start, end, content)`
    pub fn update(&mut self, start: usize, end: usize, content: &str) {
        self.update_impl(start, end, content, false);
    }

    fn update_impl(&mut self, start: usize, end: usize, content: &str, overwrite: bool) {
        assert!(start != end, "Cannot overwrite a zero-length range");
        self.split(start);
        self.split(end);
        let keys: Vec<usize> = self.chunks.range(start..end).map(|(k, _)| *k).collect();
        for (i, k) in keys.iter().enumerate() {
            let c = self.chunks.get_mut(k).unwrap();
            if i == 0 {
                c.content = Some(content.to_string());
                if overwrite {
                    c.intro.clear();
                    c.outro.clear();
                }
            } else {
                c.content = Some(String::new());
                c.intro.clear();
                c.outro.clear();
            }
            c.edited = true;
        }
    }

    pub fn remove(&mut self, start: usize, end: usize) {
        if start == end {
            return;
        }
        self.split(start);
        self.split(end);
        let keys: Vec<usize> = self.chunks.range(start..end).map(|(k, _)| *k).collect();
        for k in keys {
            let c = self.chunks.get_mut(&k).unwrap();
            c.intro.clear();
            c.outro.clear();
            c.content = Some(String::new());
            c.edited = true;
        }
    }
}

impl std::fmt::Display for MagicString<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.intro)?;
        for (start, c) in &self.chunks {
            f.write_str(&c.intro)?;
            match &c.content {
                Some(s) => f.write_str(s)?,
                None => f.write_str(&self.original[*start..c.end])?,
            }
            f.write_str(&c.outro)?;
        }
        f.write_str(&self.outro)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_and_removes() {
        let mut s = MagicString::new("abcdef");
        s.prepend_right(2, "X");
        s.append_left(2, "Y");
        assert_eq!(s.to_string(), "abYXcdef");
        s.remove(2, 4);
        assert_eq!(s.to_string(), "abYef");
        s.update(4, 5, "Z");
        assert_eq!(s.to_string(), "abYZf");
    }
}
