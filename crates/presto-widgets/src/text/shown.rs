//! The string a field lays out and draws (its "shown" text), and the byte mapping between it and the
//! document: a password field shows one mask character per character, and an IME composition shows
//! its preedit at the caret without being part of the document (quire-text's `TextView` preedit).

use std::ops::Range;

/// A field's shown text and its mapping to document bytes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shown {
    pub text: String,
    /// The mask character's UTF-8 length; 0 when unmasked.
    mask_len: usize,
    /// Masked: the document byte of every char, in order (the mapping by char index).
    bounds: Vec<usize>,
    doc_len: usize,
    /// The preedit: the document byte it sits at and its shown range.
    preedit: Option<(usize, Range<usize>)>,
}

impl Shown {
    /// `doc`'s shown text: masked with `mask` (if any), with `preedit` spliced in at document byte
    /// `.0` (masked too).
    pub fn build(doc: &str, mask: Option<char>, preedit: Option<(usize, &str)>) -> Shown {
        let preedit = preedit.filter(|p| !p.1.is_empty()).map(|(at, t)| (super::utf16::floor_boundary(doc, at), t));
        match mask {
            None => {
                let mut text = String::with_capacity(doc.len() + preedit.map_or(0, |p| p.1.len()));
                let pre = preedit.map(|(at, t)| {
                    text.push_str(&doc[..at]);
                    text.push_str(t);
                    text.push_str(&doc[at..]);
                    (at, at..at + t.len())
                });
                if pre.is_none() {
                    text.push_str(doc);
                }
                Shown { text, mask_len: 0, bounds: Vec::new(), doc_len: doc.len(), preedit: pre }
            }
            Some(m) => {
                let ml = m.len_utf8();
                let bounds: Vec<usize> = doc.char_indices().map(|(i, _)| i).collect();
                let mut text = String::new();
                let mut pre = None;
                for &b in &bounds {
                    if let Some((at, t)) = preedit
                        && at == b
                    {
                        let s = text.len();
                        (0..t.chars().count()).for_each(|_| text.push(m));
                        pre = Some((at, s..text.len()));
                    }
                    text.push(m);
                }
                if let Some((at, t)) = preedit
                    && pre.is_none()
                {
                    let s = text.len();
                    (0..t.chars().count()).for_each(|_| text.push(m));
                    pre = Some((at, s..text.len()));
                }
                Shown { text, mask_len: ml, bounds, doc_len: doc.len(), preedit: pre }
            }
        }
    }

    /// The preedit's shown range.
    pub fn preedit_range(&self) -> Option<Range<usize>> {
        self.preedit.as_ref().map(|p| p.1.clone())
    }

    /// The shown byte of document byte `b` (before the preedit when `b` is where it sits).
    pub fn to_shown(&self, b: usize) -> usize {
        let b = b.min(self.doc_len);
        let s = if self.mask_len > 0 { self.bounds.partition_point(|&x| x < b) * self.mask_len } else { b };
        match &self.preedit {
            Some((at, r)) if b > *at => s + r.len(),
            _ => s,
        }
    }

    /// The document byte of shown byte `s` (a byte inside the preedit maps to where it sits).
    pub fn to_doc(&self, s: usize) -> usize {
        let mut s = s.min(self.text.len());
        if let Some((at, r)) = &self.preedit {
            if s >= r.start && s <= r.end {
                return *at;
            }
            if s > r.end {
                s -= r.len();
            }
        }
        if self.mask_len > 0 {
            let k = s / self.mask_len;
            self.bounds.get(k).copied().unwrap_or(self.doc_len)
        } else {
            s
        }
    }

    /// A document range in shown bytes.
    pub fn range_to_shown(&self, r: Range<usize>) -> Range<usize> {
        self.to_shown(r.start)..self.to_shown(r.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preedit_and_mask_mapping() {
        let s = Shown::build("héllo", None, Some((3, "かな")));
        assert_eq!(s.text, "héかなllo");
        assert_eq!(s.preedit_range(), Some(3..9));
        assert_eq!(s.to_shown(3), 3);
        assert_eq!(s.to_shown(4), 10);
        assert_eq!(s.to_doc(5), 3);
        assert_eq!(s.to_doc(10), 4);

        let m = Shown::build("aé😀", Some('•'), None);
        assert_eq!(m.text, "•••");
        assert_eq!(m.to_shown(1), 3);
        assert_eq!(m.to_shown(3), 6);
        assert_eq!(m.to_shown(7), 9);
        assert_eq!(m.to_doc(6), 3);
        assert_eq!(m.to_doc(9), 7);

        let mp = Shown::build("ab", Some('*'), Some((2, "xy")));
        assert_eq!(mp.text, "****");
        assert_eq!(mp.preedit_range(), Some(2..4));
        assert_eq!(mp.to_doc(3), 2);
    }
}
