//! Fix-its that need the source text (CX1), derived by [`Diagnostics::finish`](crate::diag::Diagnostics::finish).
//!
//! The reporting site knows *what* would fix a diagnostic ([`FixHint`]: the "did you mean" names,
//! a modifier to remove) or the code implies it (a `bind:` whose help says to set the property one
//! way); this module finds *where* in the text. A rule that cannot place its edit exactly offers no
//! fix-it rather than a guess. Fix-its set where the diagnostic is reported (FW1's modifiers,
//! `!important`) are kept as they are.
//!
//! Every fix-it must make its diagnostic go away when applied (the `cx1_check` tests apply each
//! one and check again).

use crate::diag::{Diagnostic, FixHint, FixIt, TextEdit};

/// At most this many "did you mean" fix-its per diagnostic.
const MAX_SUGGESTIONS: usize = 3;

/// Directive prefixes: a "did you mean" on `bind:valu={v}` replaces the directive's name.
const DIRECTIVES: &[&str] = &["bind", "on", "use", "class", "style", "transition", "in", "out", "animate", "let"];

pub(crate) fn derive(src: &str, d: &mut Diagnostic) {
    let (start, end) = (d.start as usize, d.end as usize);
    let Some(text) = src.get(start..end) else { return };
    match std::mem::take(&mut d.hint) {
        FixHint::Suggest(names) => suggest(src, d, &names),
        FixHint::Remove(t) => {
            if let Some(i) = text.find(&t) {
                let at = d.start + i as u32;
                d.fixes.push(FixIt::new(format!("remove `{t}`"), vec![edit(at, at + t.len() as u32, "")]));
            }
        }
        FixHint::None => {}
    }
    match d.code {
        "read_only_property" | "bind_set_only" | "bind_not_observable" | "nonreactive_redundant" => one_way(d, text),
        // CX3: a `bind:x|commit` the kind cannot commit: drop the modifier (bind on every change)
        "bind_commit_unsupported" => {
            if let Some(i) = text.find("|commit") {
                let at = d.start + i as u32;
                d.fixes.push(FixIt::new("remove `|commit`", vec![edit(at, at + 7, "")]));
            }
        }
        // U4: `class:w-1/2={x}` stops the parser at the `/`
        "expected_token" => class_with_slash(src, d),
        "block_invalid_elseif" => {
            let window = &src[start..src.len().min(start + 16)];
            if let Some(i) = window.find("elseif") {
                let at = d.start + i as u32;
                d.fixes.push(FixIt::new("replace `elseif` with `else if`", vec![edit(at, at + 6, "else if")]));
            }
        }
        _ => {}
    }
}

fn edit(start: u32, end: u32, text: &str) -> TextEdit {
    TextEdit { start, end, text: text.to_string() }
}

fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '$'
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | ':' | '.' | '-')
}

/// The length of the name at the start of `s` (letters, digits, `_$:.-`; first a letter, `_` or `$`).
fn name_len(s: &str) -> usize {
    let mut chars = s.char_indices();
    match chars.next() {
        Some((_, c)) if is_name_start(c) => {}
        _ => return 0,
    }
    chars.find(|&(_, c)| !is_name_char(c)).map_or(s.len(), |(i, _)| i)
}

/// "did you mean": replace the misspelled name at the span with each suggestion. The span is the
/// name itself (an attribute, `onclickd`, `parent:x`), an element's start tag (`<buton`: the end
/// tag is renamed too), or a directive (`bind:valu={v}`: the directive's name).
fn suggest(src: &str, d: &mut Diagnostic, names: &[String]) {
    let text = &src[d.start as usize..d.end as usize];
    let n = name_len(text);
    let directive = text.find(':').filter(|&c| DIRECTIVES.contains(&&text[..c]));
    // the places the name is written, and the directive prefix (`bind`, `on`, ...) if any
    let (places, prefix): (Vec<(u32, u32)>, &str) = if let Some(colon) = directive {
        let rest = &text[colon + 1..];
        let n = rest.find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | '-'))).unwrap_or(rest.len());
        if n == 0 {
            return;
        }
        if &text[..colon] == "bind" && !rest[n..].trim_start().starts_with(['=', '|']) {
            // the shorthand `bind:x` binds the variable `x`: keep it
            let var = &rest[..n];
            for s in names.iter().take(MAX_SUGGESTIONS).filter(|s| name_len(s) == s.len()) {
                d.fixes.push(FixIt::new(format!("replace with `{s}`"), vec![edit(d.start, d.start + colon as u32 + 1 + n as u32, &format!("bind:{s}={{{var}}}"))]));
            }
            return;
        }
        let at = d.start + colon as u32 + 1;
        (vec![(at, at + n as u32)], &text[..colon])
    } else if n > 0 && n == text.len() {
        let mut places = vec![(d.start, d.end)];
        // an element's name (`<buton`): its end tag too
        if d.start > 0
            && src.as_bytes()[d.start as usize - 1] == b'<'
            && let Some(close) = end_tag(src, d.end as usize, text)
        {
            places.push(close);
        }
        (places, "")
    } else if let Some(rest) = text.strip_prefix('<') {
        let n = name_len(rest);
        if n == 0 {
            return;
        }
        let name = &rest[..n];
        let open = (d.start + 1, d.start + 1 + n as u32);
        let mut places = vec![open];
        if let Some(close) = end_tag(src, d.start as usize + 1 + n, name) {
            places.push(close);
        }
        (places, "")
    } else {
        return;
    };
    for s in names.iter().take(MAX_SUGGESTIONS) {
        // `on:clickd` with the suggestion `onclicked` is `on:clicked`
        let s = if prefix == "on" { s.strip_prefix("on").unwrap_or(s) } else { s.as_str() };
        if s.is_empty() || s.contains(char::is_whitespace) {
            continue;
        }
        let edits = places.iter().map(|&(a, b)| edit(a, b, s)).collect();
        d.fixes.push(FixIt::new(format!("replace with `{s}`"), edits));
    }
}

/// The span of the name in the end tag of the element whose start tag's name ends at `from`:
/// `</name>`, skipping nested elements of the same name. `None` when the element is self-closing
/// or no end tag is found.
fn end_tag(src: &str, from: usize, name: &str) -> Option<(u32, u32)> {
    let b = src.as_bytes();
    let boundary = |i: usize| i >= b.len() || !is_name_char(b[i] as char);
    // the start tag: self-closing when it ends with `/>`
    let gt = src[from..].find('>')? + from;
    if b[gt - 1] == b'/' {
        return None;
    }
    let mut depth = 0usize;
    let mut i = gt + 1;
    while let Some(off) = src[i..].find('<') {
        let at = i + off;
        if src[at + 1..].starts_with('/') && src[at + 2..].starts_with(name) && boundary(at + 2 + name.len()) {
            if depth == 0 {
                return Some(((at + 2) as u32, (at + 2 + name.len()) as u32));
            }
            depth -= 1;
        } else if src[at + 1..].starts_with(name) && boundary(at + 1 + name.len()) {
            let gt = src[at..].find('>').map(|g| g + at)?;
            if b[gt - 1] != b'/' {
                depth += 1;
            }
        }
        i = at + 1;
    }
    None
}

/// `bind:x={v}` where the help says to set the property one way (`` `prop={...}` ``): the
/// directive becomes `prop={v}` (`bind:x` alone, the shorthand of `bind:x={x}`, becomes `prop={x}`).
fn one_way(d: &mut Diagnostic, text: &str) {
    let Some(rest) = text.strip_prefix("bind:") else { return };
    let Some(help) = d.help.as_deref() else { return };
    let Some(i) = help.find("={...}`") else { return };
    let Some(tick) = help[..i].rfind('`') else { return };
    let prop = &help[tick + 1..i];
    if prop.is_empty() || name_len(prop) != prop.len() {
        return;
    }
    let n = rest.find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | '-'))).unwrap_or(rest.len());
    let name = &rest[..n];
    let value = match rest.find('=') {
        Some(eq) => rest[eq + 1..].to_string(),
        None => format!("{{{name}}}"),
    };
    d.fixes.push(FixIt::new(format!("set `{prop}` one way"), vec![edit(d.start, d.end, &format!("{prop}={value}"))]));
}

/// U4 (UTILITY-CSS §4.6): a `class:` directive whose name has a `/` (`class:w-1/2={x}`,
/// `class:bg-accent/50={x}`) is not valid Svelte: the parser ends the name at the `/` and then
/// expects `>`. The fix-it rewrites it as a clsx object, `class={{ 'w-1/2': x }}`, when the start
/// tag has no other `class` attribute.
fn class_with_slash(src: &str, d: &mut Diagnostic) {
    let b = src.as_bytes();
    let at = d.start as usize;
    if at == 0 || at > b.len() || b[at - 1] != b'/' {
        return;
    }
    // the attribute before the `/`: `class:<name>`
    let start = src[..at - 1].rfind(|c: char| c.is_whitespace() || c == '<').map_or(0, |i| i + 1);
    let Some(first) = src[start..at - 1].strip_prefix("class:") else { return };
    if first.is_empty() {
        return;
    }
    // the rest of the name: up to `=`, white space or the end of the tag
    let mut k = at;
    while k < b.len() && !matches!(b[k], b'=' | b'>' | b'{' | b'"' | b'\'') && !b[k].is_ascii_whitespace() {
        k += 1;
    }
    let name = &src[start + "class:".len()..k];
    if name.ends_with('/') || !src[k..].starts_with("={") {
        return;
    }
    // its value: `={expr}`
    let open = k + 1;
    let (mut depth, mut quote, mut i, mut close) = (0i32, 0u8, open, None);
    while i < b.len() {
        let c = b[i];
        if quote != 0 {
            if c == b'\\' {
                i += 1;
            } else if c == quote {
                quote = 0;
            }
        } else if matches!(c, b'"' | b'\'' | b'`') {
            quote = c;
        } else if c == b'{' {
            depth += 1;
        } else if c == b'}' {
            depth -= 1;
            if depth == 0 {
                close = Some(i);
                break;
            }
        }
        i += 1;
    }
    let Some(close) = close else { return };
    let expr = src[open + 1..close].trim();
    if expr.is_empty() {
        return;
    }
    // no other `class` attribute in the start tag
    let tag_start = src[..start].rfind('<').unwrap_or(0);
    let tag_end = src[close..].find('>').map_or(src.len(), |e| close + e);
    let other = |s: &str| s.split(char::is_whitespace).any(|w| w == "class" || w.starts_with("class=") || w.starts_with("class:"));
    if other(&src[tag_start..start]) || other(&src[close + 1..tag_end]) {
        return;
    }
    let text = format!("class={{{{ '{name}': {expr} }}}}");
    d.fixes.push(FixIt::new(format!("write it as `{text}`"), vec![edit(start as u32, close as u32 + 1, &text)]));
}
