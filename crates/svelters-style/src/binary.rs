//! Precompiled sheets (skelter `css_precompiled`, compiler-and-js.md §3.7): a component's scoped
//! sheet parsed at compile time and written as bytes, with what the compiler knows about its
//! elements, so loading skips the sheet parser and restyling tests fewer rules.
//!
//! A [`Precompiled`] holds:
//! - the [`Stylesheet`]: rules with their selector text (nesting already resolved), media, layer
//!   and position; declaration blocks interned (identical blocks are stored once and shared);
//! - [`ElementHints`] per template element (by its source offset): the rules Svelte's CSS pruning
//!   says might apply to it (`candidates`) and the ones that match it whatever its context
//!   (`always`: compound selectors of type, class and id over its static attributes);
//! - `global_rules`: rules that may match elements of other components (`:global`), which every
//!   element still tests;
//! - the resolved [`Utility`] rules of the component's static utility classes.
//!
//! [`decode`] parses only the selectors (and media queries) again, from their text.
//!
//! Format (all integers LEB128 `u32`): `b"SVSB"`, version, the string table (length-prefixed
//! UTF-8), the layers, the declaration blocks, the rules, the hints, the global rules, the
//! utilities. Strings are referenced by index.

use crate::atom::Atom;
use crate::media::MediaList;
use crate::selector::parse_selector_list;
use crate::sheet::{DeclBlock, Declaration, ParseOptions, Rule, Stylesheet};
use crate::utility::Utility;
use std::collections::HashMap;
use std::sync::Arc;

const MAGIC: &[u8; 4] = b"SVSB";
const VERSION: u32 = 1;

/// What the compiler knows about one template element.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ElementHints {
    /// The element's start offset in its `.svelte` source.
    pub element: u32,
    /// Indices of the rules that might match it (Svelte's pruning), in rule order.
    pub candidates: Vec<u32>,
    /// The candidates that match it in any context (a subset of `candidates`).
    pub always: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct Precompiled {
    pub sheet: Stylesheet,
    pub hints: Vec<ElementHints>,
    pub global_rules: Vec<u32>,
    pub utilities: Vec<Utility>,
}

#[derive(Default)]
struct Writer {
    strings: Vec<String>,
    index: HashMap<String, u32>,
}

impl Writer {
    fn u32(out: &mut Vec<u8>, mut v: u32) {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                out.push(b);
                return;
            }
            out.push(b | 0x80);
        }
    }

    fn str(&mut self, s: &str) -> u32 {
        if let Some(&i) = self.index.get(s) {
            return i;
        }
        let i = self.strings.len() as u32;
        self.strings.push(s.to_string());
        self.index.insert(s.to_string(), i);
        i
    }
}

/// The interning key of a declaration block: its declarations without positions.
fn block_key(b: &DeclBlock) -> Vec<(&str, &str, bool, bool)> {
    b.decls.iter().map(|d| (&*d.name, &*d.value, d.important, d.has_var)).collect()
}

/// Writes `p` as bytes.
pub fn encode(p: &Precompiled) -> Vec<u8> {
    let mut w = Writer::default();
    let mut body = Vec::new();
    let put = Writer::u32;

    put(&mut body, p.sheet.layers.len() as u32);
    for l in &p.sheet.layers {
        let i = w.str(l);
        put(&mut body, i);
    }

    // interned blocks
    let mut blocks: Vec<&DeclBlock> = Vec::new();
    let mut block_ix: HashMap<Vec<(&str, &str, bool, bool)>, u32> = HashMap::new();
    let mut rule_block = Vec::with_capacity(p.sheet.rules.len());
    for r in &p.sheet.rules {
        let key = block_key(&r.block);
        let ix = *block_ix.entry(key).or_insert_with(|| {
            blocks.push(&r.block);
            blocks.len() as u32 - 1
        });
        rule_block.push(ix);
    }
    put(&mut body, blocks.len() as u32);
    for b in &blocks {
        put(&mut body, b.decls.len() as u32);
        for d in &b.decls {
            let (n, v) = (w.str(&d.name), w.str(&d.value));
            put(&mut body, n);
            put(&mut body, v);
            body.push(d.important as u8 | (d.has_var as u8) << 1);
            put(&mut body, d.line);
            put(&mut body, d.column);
        }
    }

    put(&mut body, p.sheet.rules.len() as u32);
    for (r, &b) in p.sheet.rules.iter().zip(&rule_block) {
        let s = w.str(&r.selector_text());
        put(&mut body, s);
        put(&mut body, b);
        put(&mut body, r.media.len() as u32);
        for m in &r.media {
            let i = w.str(&m.text);
            put(&mut body, i);
        }
        let layer = r.layer.as_ref().map_or(0, |l| w.str(l) + 1);
        put(&mut body, layer);
        put(&mut body, r.line);
        put(&mut body, r.column);
    }

    put(&mut body, p.hints.len() as u32);
    for h in &p.hints {
        put(&mut body, h.element);
        for list in [&h.candidates, &h.always] {
            put(&mut body, list.len() as u32);
            for &x in list {
                put(&mut body, x);
            }
        }
    }
    put(&mut body, p.global_rules.len() as u32);
    for &x in &p.global_rules {
        put(&mut body, x);
    }

    put(&mut body, p.utilities.len() as u32);
    for u in &p.utilities {
        let (c, s) = (w.str(&u.class), w.str(&u.selector));
        put(&mut body, c);
        put(&mut body, s);
        put(&mut body, u.media.len() as u32);
        for m in &u.media {
            let i = w.str(m);
            put(&mut body, i);
        }
        put(&mut body, u.decls.len() as u32);
        for (n, v) in &u.decls {
            let (n, v) = (w.str(n), w.str(v));
            put(&mut body, n);
            put(&mut body, v);
        }
        body.push(u.important as u8);
        put(&mut body, u.variant_rank as u32);
        put(&mut body, u.row_rank as u32);
    }

    let mut out = Vec::with_capacity(body.len() + 64);
    out.extend_from_slice(MAGIC);
    put(&mut out, VERSION);
    put(&mut out, w.strings.len() as u32);
    for s in &w.strings {
        put(&mut out, s.len() as u32);
        out.extend_from_slice(s.as_bytes());
    }
    out.extend_from_slice(&body);
    out
}

struct Reader<'b> {
    bytes: &'b [u8],
    at: usize,
    strings: Vec<&'b str>,
}

impl<'b> Reader<'b> {
    fn u32(&mut self) -> Result<u32, String> {
        let mut v: u32 = 0;
        for shift in (0..35).step_by(7) {
            let b = *self.bytes.get(self.at).ok_or("precompiled sheet: truncated")?;
            self.at += 1;
            v |= ((b & 0x7f) as u32) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err("precompiled sheet: bad integer".into())
    }

    fn byte(&mut self) -> Result<u8, String> {
        let b = *self.bytes.get(self.at).ok_or("precompiled sheet: truncated")?;
        self.at += 1;
        Ok(b)
    }

    fn str(&mut self) -> Result<&'b str, String> {
        let i = self.u32()? as usize;
        self.strings.get(i).copied().ok_or_else(|| format!("precompiled sheet: string {i} out of range"))
    }

    fn list(&mut self) -> Result<Vec<u32>, String> {
        let n = self.u32()?;
        (0..n).map(|_| self.u32()).collect()
    }
}

/// Gives anonymous layers (`\0anon<n>` segments) fresh names, as parsing the text again would: two
/// precompiled sheets never share one.
fn fresh_layer(name: &str, renames: &mut HashMap<String, String>) -> Atom {
    if !name.contains('\0') {
        return Atom::new(name);
    }
    let parts: Vec<String> = name.split('.').map(|seg| if seg.starts_with('\0') { renames.entry(seg.to_string()).or_insert_with(crate::sheet::anon_layer_name).clone() } else { seg.to_string() }).collect();
    Atom::new(&parts.join("."))
}

/// Reads a [`Precompiled`]; `opts` resolves `:state()` in the selectors as the sheet parser would.
pub fn decode(bytes: &[u8], opts: &ParseOptions<'_>) -> Result<Precompiled, String> {
    if bytes.len() < 5 || &bytes[..4] != MAGIC {
        return Err("not a precompiled sheet".into());
    }
    let mut r = Reader { bytes, at: 4, strings: Vec::new() };
    let version = r.u32()?;
    if version != VERSION {
        return Err(format!("precompiled sheet version {version}, this engine reads {VERSION}: recompile"));
    }
    let n = r.u32()?;
    for _ in 0..n {
        let len = r.u32()? as usize;
        let end = r.at.checked_add(len).filter(|&e| e <= bytes.len()).ok_or("precompiled sheet: truncated string")?;
        let s = std::str::from_utf8(&bytes[r.at..end]).map_err(|e| format!("precompiled sheet: {e}"))?;
        r.strings.push(s);
        r.at = end;
    }

    let mut renames = HashMap::new();
    let layer_count = r.u32()?;
    let mut layers = Vec::with_capacity(layer_count as usize);
    for _ in 0..layer_count {
        let s = r.str()?;
        layers.push(fresh_layer(s, &mut renames));
    }

    let block_count = r.u32()?;
    let mut blocks = Vec::with_capacity(block_count as usize);
    for _ in 0..block_count {
        let n = r.u32()?;
        let mut decls = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let name = r.str()?;
            let value = r.str()?;
            let flags = r.byte()?;
            let line = r.u32()?;
            let column = r.u32()?;
            decls.push(Declaration { name: Atom::new(name), value: value.into(), important: flags & 1 != 0, has_var: flags & 2 != 0, line, column });
        }
        blocks.push(Arc::new(DeclBlock { decls }));
    }

    let mut media_cache: HashMap<&str, Arc<MediaList>> = HashMap::new();
    let rule_count = r.u32()?;
    let mut rules = Vec::with_capacity(rule_count as usize);
    for _ in 0..rule_count {
        let sel = r.str()?;
        let selectors = parse_selector_list(sel, opts.states).map_err(|e| format!("precompiled sheet: selector `{sel}`: {e}"))?;
        let b = r.u32()? as usize;
        let block = blocks.get(b).cloned().ok_or("precompiled sheet: block out of range")?;
        let mn = r.u32()?;
        let mut media = Vec::with_capacity(mn as usize);
        for _ in 0..mn {
            let text = r.str()?;
            let m = match media_cache.get(text) {
                Some(m) => m.clone(),
                None => {
                    let m = Arc::new(MediaList::parse(text).map_err(|e| format!("precompiled sheet: @media {text}: {e}"))?);
                    media_cache.insert(text, m.clone());
                    m
                }
            };
            media.push(m);
        }
        let layer = match r.u32()? {
            0 => None,
            i => Some(fresh_layer(r.strings.get(i as usize - 1).ok_or("precompiled sheet: layer out of range")?, &mut renames)),
        };
        let line = r.u32()?;
        let column = r.u32()?;
        rules.push(Rule { selectors, block, media, layer, line, column });
    }

    let hint_count = r.u32()?;
    let mut hints = Vec::with_capacity(hint_count as usize);
    for _ in 0..hint_count {
        let element = r.u32()?;
        let candidates = r.list()?;
        let always = r.list()?;
        hints.push(ElementHints { element, candidates, always });
    }
    let global_rules = r.list()?;

    let util_count = r.u32()?;
    let mut utilities = Vec::with_capacity(util_count as usize);
    for _ in 0..util_count {
        let class = Atom::new(r.str()?);
        let selector = r.str()?.to_string();
        let mn = r.u32()?;
        let media = (0..mn).map(|_| r.str().map(str::to_string)).collect::<Result<Vec<_>, _>>()?;
        let dn = r.u32()?;
        let decls = (0..dn).map(|_| Ok((Atom::new(r.str()?), r.str()?.to_string()))).collect::<Result<Vec<_>, String>>()?;
        let important = r.byte()? != 0;
        let variant_rank = r.u32()? as u16;
        let row_rank = r.u32()? as u16;
        utilities.push(Utility { class, selector, media, decls, important, variant_rank, row_rank, notes: Vec::new() });
    }
    if r.at != bytes.len() {
        return Err("precompiled sheet: trailing bytes".into());
    }
    Ok(Precompiled { sheet: Stylesheet { rules, layers, diagnostics: Vec::new() }, hints, global_rules, utilities })
}

/// Whether a selector's text is one compound of a type, classes and an id (no combinator, no
/// pseudo-class, no attribute), and if so whether it matches an element with this tag, classes and
/// id in any context. `None`: the selector depends on more than the element's own static
/// attributes.
pub fn compound_matches(selector: &str, tag: &str, classes: &[&str], id: Option<&str>) -> Option<bool> {
    let s = selector.trim();
    if s.is_empty() || s.contains(|c: char| c.is_whitespace() || matches!(c, '>' | '+' | '~' | ':' | '[' | '\\' | '(' | ')' | '|' | ',')) {
        return None;
    }
    let mut ok = true;
    let mut rest = s;
    // the type selector, if any
    let type_end = rest.find(['.', '#']).unwrap_or(rest.len());
    let ty = &rest[..type_end];
    if !ty.is_empty() && ty != "*" && ty != tag {
        ok = false;
    }
    rest = &rest[type_end..];
    while !rest.is_empty() {
        let kind = rest.as_bytes()[0];
        let end = rest[1..].find(['.', '#']).map_or(rest.len(), |i| i + 1);
        let name = &rest[1..end];
        if name.is_empty() {
            return None;
        }
        match kind {
            b'.' => ok &= classes.contains(&name),
            b'#' => ok &= id == Some(name),
            _ => return None,
        }
        rest = &rest[end..];
    }
    Some(ok)
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding: how a precompiled sheet travels in a JS module (`registerStyles(id, { bin })`).
pub fn to_base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// [`to_base64`]'s inverse.
pub fn from_base64(text: &str) -> Result<Vec<u8>, String> {
    let text = text.trim_end_matches('=');
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for c in text.bytes() {
        let v = B64.iter().position(|&b| b == c).ok_or_else(|| format!("base64: unexpected {:?}", c as char))? as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits & 0xff) as u8);
        }
    }
    Ok(out)
}
