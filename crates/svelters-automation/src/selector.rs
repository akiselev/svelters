//! Selector strings (E2E-INSPECTOR §1.3): Playwright-style engines chained with `>>`.
//!
//! ```text
//! selector := part (">>" part)*
//! part     := "role=" ROLE ("[" attr "]")*      role=button[name="Apply"][disabled]
//!           | "label=" text | "text=" text       text: "exact" or bare (substring, any case)
//!           | "anchor=" ANCHOR | "root=" KEY     anchor=extents[0], root=panel.task
//!           | "key=" ITEM                        the keyed each item `ITEM`: key=k42 >> anchor=row
//!           | "pos=" X "," Y                     hit test in the root viewport (last resort)
//!           | "nth=" N                           the N-th match so far (0-based, -1: last)
//!           | "source=" FILE ("#" OFFSET)?       DZ0: nodes a dev build declared in FILE (a path or
//!                                                a suffix of one at a `/`), at that byte offset
//!           | "component=" … | "a11y=" …          parsed; answered `capability.absent` (AU3)
//!           | "css=" CSS | CSS                   the svelters-style matcher (the default engine)
//! attr     := "name" ("=" | "*=") text | STATE   STATE: a state pseudo-class name (`checked`,
//!                                                `disabled`, `focus`, `selected`, …)
//! ```
//!
//! `>>` inside quotes or brackets is not a separator.

use crate::Error;

/// How a text value matches: `"Exact"` (quoted) or a bare substring that ignores case
/// (Playwright's `text=` rules). Regular expressions come later (they need the `regex` crate).
#[derive(Clone, Debug, PartialEq)]
pub enum TextMatch {
    Exact(String),
    Contains(String),
}

impl TextMatch {
    pub fn matches(&self, s: &str) -> bool {
        match self {
            TextMatch::Exact(t) => s.trim() == t,
            TextMatch::Contains(t) => s.to_lowercase().contains(&t.to_lowercase()),
        }
    }

    fn parse(v: &str) -> Result<TextMatch, Error> {
        let v = v.trim();
        if v.starts_with('/') {
            return Err(Error::new(
                "selector.invalid",
                format!("regular expressions are not supported yet: {v}"),
            ));
        }
        if v.len() >= 2
            && (v.starts_with('"') && v.ends_with('"') || v.starts_with('\'') && v.ends_with('\''))
        {
            return Ok(TextMatch::Exact(unescape(&v[1..v.len() - 1])));
        }
        Ok(TextMatch::Contains(v.to_string()))
    }
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            if let Some(n) = it.next() {
                out.push(n);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// One engine step of a selector.
#[derive(Clone, Debug)]
pub enum Part {
    Role {
        role: String,
        name: Option<TextMatch>,
        states: Vec<String>,
    },
    Label(TextMatch),
    Text(TextMatch),
    Anchor(String),
    /// CV1 (SV53): the top nodes of the keyed `{#each}` item with this key (the next part searches
    /// them and their subtrees).
    Key(String),
    Root(String),
    Pos(crate::geom::Pos2),
    Nth(i64),
    Css(String),
    /// DZ0: the nodes declared in `file` (matched as a path suffix at a `/`), at `offset` if given.
    Source { file: String, offset: Option<u32> },
    /// Engines whose data lands later (`component=` in AU3, `a11y=` with AU2's tree).
    Deferred(&'static str),
}

/// A parsed selector: the parts, applied left to right, each within the matches of the last.
#[derive(Clone, Debug)]
pub struct Selector {
    pub source: String,
    pub parts: Vec<Part>,
}

impl std::str::FromStr for Selector {
    type Err = Error;
    fn from_str(s: &str) -> Result<Selector, Error> {
        parse(s)
    }
}

/// Parses `s` (see the module docs).
pub fn parse(s: &str) -> Result<Selector, Error> {
    let parts = split_chain(s)
        .into_iter()
        .map(|p| part(p.trim()))
        .collect::<Result<Vec<_>, _>>()?;
    if parts.is_empty() {
        return Err(Error::new("selector.invalid", "empty selector"));
    }
    Ok(Selector {
        source: s.to_string(),
        parts,
    })
}

fn split_chain(s: &str) -> Vec<&str> {
    let (mut out, mut start, mut depth, mut quote) = (Vec::new(), 0, 0i32, None::<u8>);
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) if c == b'\\' => i += 1,
            Some(_) => {}
            None => match c {
                b'"' | b'\'' => quote = Some(c),
                b'[' | b'(' => depth += 1,
                b']' | b')' => depth -= 1,
                b'>' if depth == 0 && b.get(i + 1) == Some(&b'>') => {
                    out.push(&s[start..i]);
                    start = i + 2;
                    i += 1;
                }
                _ => {}
            },
        }
        i += 1;
    }
    out.push(&s[start..]);
    out.into_iter().filter(|p| !p.trim().is_empty()).collect()
}

fn part(p: &str) -> Result<Part, Error> {
    let Some((engine, v)) = p
        .split_once('=')
        .filter(|(e, _)| !e.is_empty() && e.bytes().all(|c| c.is_ascii_alphanumeric()))
    else {
        return Ok(Part::Css(p.to_string()));
    };
    Ok(match engine {
        "role" => role(v)?,
        "label" => Part::Label(TextMatch::parse(v)?),
        "text" => Part::Text(TextMatch::parse(v)?),
        "anchor" => Part::Anchor(strip_quotes(v).to_string()),
        "key" => Part::Key(strip_quotes(v).to_string()),
        "root" => Part::Root(strip_quotes(v).to_string()),
        "css" => Part::Css(v.to_string()),
        "nth" => {
            Part::Nth(v.trim().parse().map_err(|_| {
                Error::new("selector.invalid", format!("nth wants an integer: {v}"))
            })?)
        }
        "pos" => {
            let (x, y) = v
                .split_once(',')
                .ok_or_else(|| Error::new("selector.invalid", format!("pos wants x,y: {v}")))?;
            let f = |t: &str| {
                t.trim()
                    .parse::<f32>()
                    .map_err(|_| Error::new("selector.invalid", format!("pos wants numbers: {v}")))
            };
            Part::Pos(crate::geom::pos2(f(x)?, f(y)?))
        }
        "component" => Part::Deferred("component"),
        "source" => source(strip_quotes(v))?,
        "a11y" => Part::Deferred("a11y"),
        // `[value=3]` and other CSS that contains `=` reach here only with an engine-like prefix
        _ => return Ok(Part::Css(p.to_string())),
    })
}

fn source(v: &str) -> Result<Part, Error> {
    let (file, offset) = match v.rsplit_once('#') {
        Some((f, o)) => {
            let o = o.trim().parse().map_err(|_| {
                Error::new("selector.invalid", format!("source wants FILE#OFFSET (a byte offset): {v}"))
            })?;
            (f, Some(o))
        }
        None => (v, None),
    };
    if file.is_empty() {
        return Err(Error::new("selector.invalid", "source wants a file"));
    }
    Ok(Part::Source { file: file.to_string(), offset })
}

/// DZ0: `path` is `want` or ends with `/want`.
pub fn path_matches(path: &str, want: &str) -> bool {
    path == want || path.strip_suffix(want).is_some_and(|p| p.ends_with('/') || want.starts_with('/'))
}

fn strip_quotes(v: &str) -> &str {
    let v = v.trim();
    if v.len() >= 2
        && (v.starts_with('"') && v.ends_with('"') || v.starts_with('\'') && v.ends_with('\''))
    {
        &v[1..v.len() - 1]
    } else {
        v
    }
}

fn role(v: &str) -> Result<Part, Error> {
    let v = v.trim();
    let (role, mut rest) = match v.find('[') {
        Some(i) => (&v[..i], &v[i..]),
        None => (v, ""),
    };
    let (mut name, mut states) = (None, Vec::new());
    while !rest.is_empty() {
        let close = close_bracket(rest)
            .ok_or_else(|| Error::new("selector.invalid", format!("unclosed `[` in role={v}")))?;
        let attr = rest[1..close].trim();
        rest = rest[close + 1..].trim_start();
        if let Some(t) = attr.strip_prefix("name") {
            let t = t.trim_start();
            if let Some(t) = t.strip_prefix("*=") {
                name = Some(TextMatch::Contains(strip_quotes(t).to_string()));
            } else if let Some(t) = t.strip_prefix('=') {
                name = Some(TextMatch::parse(t)?);
            } else {
                return Err(Error::new(
                    "selector.invalid",
                    format!("role attribute `{attr}`: use name=\"…\""),
                ));
            }
        } else {
            states.push(attr.to_string());
        }
    }
    if role.is_empty() {
        return Err(Error::new("selector.invalid", "role= needs a role"));
    }
    Ok(Part::Role {
        role: role.to_string(),
        name,
        states,
    })
}

fn close_bracket(s: &str) -> Option<usize> {
    let mut quote = None;
    for (i, c) in s.char_indices().skip(1) {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == ']' => return Some(i),
            None => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chains_and_engines() {
        let s = parse(r#"root=panel.task >> role=button[name="Apply >> now"][disabled] >> nth=-1"#)
            .unwrap();
        assert_eq!(s.parts.len(), 3);
        match &s.parts[1] {
            Part::Role { role, name, states } => {
                assert_eq!(role, "button");
                assert_eq!(name, &Some(TextMatch::Exact("Apply >> now".into())));
                assert_eq!(states, &vec!["disabled".to_string()]);
            }
            p => panic!("{p:?}"),
        }
        assert!(matches!(s.parts[2], Part::Nth(-1)));
        assert!(
            matches!(&parse("button.primary > label").unwrap().parts[0], Part::Css(c) if c == "button.primary > label")
        );
        assert!(
            matches!(&parse("text=count").unwrap().parts[0], Part::Text(TextMatch::Contains(t)) if t == "count")
        );
        assert!(matches!(&parse("pos=10,20").unwrap().parts[0], Part::Pos(p) if p.x == 10.0));
        assert!(matches!(
            &parse("component=TaskPanel").unwrap().parts[0],
            Part::Deferred("component")
        ));
        assert!(matches!(
            &parse("source=panels/Jobs.svelte#120").unwrap().parts[0],
            Part::Source { file, offset: Some(120) } if file == "panels/Jobs.svelte"
        ));
        assert!(matches!(
            &parse("source=\"Jobs.svelte\"").unwrap().parts[0],
            Part::Source { file, offset: None } if file == "Jobs.svelte"
        ));
        assert!(parse("source=Jobs.svelte#x").is_err());
        assert!(path_matches("/w/ui/panels/Jobs.svelte", "panels/Jobs.svelte"));
        assert!(path_matches("/w/ui/panels/Jobs.svelte", "/w/ui/panels/Jobs.svelte"));
        assert!(!path_matches("/w/ui/panels/Jobs.svelte", "obs.svelte"));
        assert!(parse("text=/x/i").is_err());
        assert!(parse("role=button[name=\"x\"").is_err());
        assert!(TextMatch::Contains("COUNT".into()).matches("Count: 1"));
        assert!(!TextMatch::Exact("Count".into()).matches("Count: 1"));
    }
}
