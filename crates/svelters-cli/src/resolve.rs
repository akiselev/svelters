//! Address resolution (`go doc` style): `<entity>[.<member>]`, where the entity is a tag, a native
//! class name, a qualified name or an entity id, and the member is `[<kind>:]<name>`.
//!
//! Rules (plans/research/CLI.md §4.1):
//! - matching is case-insensitive; a second pass ignores `-` and `_` (`setCentralWidget` =
//!   `set-central-widget`, `close_request` = `close-request`);
//! - a member is searched on the entity, then its bases (nearest first) and interfaces, like the
//!   site's inherited blocks; per kind the first owner wins;
//! - a name that matches members of several kinds is ambiguous (exit 2) unless one of the rules in
//!   [`suppress`] applies; misses suggest near names (exit 1).

use crate::pack::{Entity, EnumValue, Event, Method, Pack, Property, Slot};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MemberKind {
    Property,
    Event,
    Method,
    Constructor,
    Value,
    Slot,
    Attr,
}

pub const ALL_KINDS: [MemberKind; 7] = [
    MemberKind::Property,
    MemberKind::Event,
    MemberKind::Slot,
    MemberKind::Attr,
    MemberKind::Method,
    MemberKind::Constructor,
    MemberKind::Value,
];

impl MemberKind {
    /// The address prefix (`prop:title`).
    pub fn prefix(self) -> &'static str {
        match self {
            MemberKind::Property => "prop",
            MemberKind::Event => "event",
            MemberKind::Method => "method",
            MemberKind::Constructor => "ctor",
            MemberKind::Value => "value",
            MemberKind::Slot => "slot",
            MemberKind::Attr => "attr",
        }
    }

    /// The word used in output and in `--json` (`"kind": "property"`).
    pub fn word(self) -> &'static str {
        match self {
            MemberKind::Property => "property",
            MemberKind::Event => "event",
            MemberKind::Method => "method",
            MemberKind::Constructor => "constructor",
            MemberKind::Value => "value",
            MemberKind::Slot => "slot",
            MemberKind::Attr => "attr",
        }
    }

    pub fn parse(s: &str) -> Option<MemberKind> {
        Some(match s.to_ascii_lowercase().as_str() {
            "prop" | "property" | "props" | "properties" => MemberKind::Property,
            "event" | "events" | "signal" => MemberKind::Event,
            "method" | "methods" => MemberKind::Method,
            "ctor" | "constructor" | "constructors" => MemberKind::Constructor,
            "value" | "values" => MemberKind::Value,
            "slot" | "slots" => MemberKind::Slot,
            "attr" | "attrs" | "child-attr" => MemberKind::Attr,
            _ => return None,
        })
    }
}

/// One resolved member. `owner` is the directory index of the declaring entity.
#[derive(Clone, Debug)]
pub enum Member<'a> {
    Property(&'a Property),
    Event(&'a Event),
    /// All overloads with this name on the owner.
    Method(Vec<&'a Method>),
    Constructor(Vec<&'a Method>),
    Value(&'a EnumValue),
    Slot(&'a Slot),
    Attr {
        slot: &'a Slot,
        attr: &'a str,
    },
}

#[derive(Clone, Debug)]
pub struct Hit<'a> {
    pub kind: MemberKind,
    pub owner: usize,
    pub member: Member<'a>,
}

impl Hit<'_> {
    /// Site anchors of the member (several for overloaded methods); empty for slots and attrs.
    pub fn anchors(&self) -> Vec<&str> {
        match &self.member {
            Member::Property(p) => vec![p.anchor.as_str()],
            Member::Event(e) => vec![e.anchor.as_str()],
            Member::Method(m) | Member::Constructor(m) => {
                m.iter().map(|x| x.anchor.as_str()).collect()
            }
            Member::Value(v) => vec![v.anchor.as_str()],
            Member::Slot(_) | Member::Attr { .. } => Vec::new(),
        }
    }

    /// The canonical member name used in addresses.
    pub fn name(&self) -> &str {
        match &self.member {
            Member::Property(p) => &p.js_name,
            Member::Event(e) => &e.js_event,
            Member::Method(m) | Member::Constructor(m) => &m[0].name,
            Member::Value(v) => &v.name,
            Member::Slot(s) => &s.name,
            Member::Attr { attr, .. } => attr,
        }
    }
}

pub fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '-' && *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

fn eq(a: &str, b: &str, loose: bool) -> bool {
    if loose {
        norm(a) == norm(b)
    } else {
        a.eq_ignore_ascii_case(b)
    }
}

/// The name an entity is addressed by: its tag, else its qualified or native name.
pub fn entity_handle(pack: &Pack, i: usize) -> String {
    let d = &pack.header.entities[i];
    d.tags
        .first()
        .cloned()
        .or_else(|| d.qualified.clone())
        .unwrap_or_else(|| d.native_name.clone())
}

pub fn entity_address(pack: &Pack, i: usize) -> String {
    format!("{} {}", pack.toolkit(), entity_handle(pack, i))
}

pub fn member_address(pack: &Pack, entity: usize, hit: &Hit) -> String {
    format!(
        "{}.{}:{}",
        entity_address(pack, entity),
        hit.kind.prefix(),
        hit.name()
    )
}

/// Directory indices of entities matching `q`, from the highest-priority key class that matches:
/// tag, entity id, qualified name, native name, short name. (`Gtk.Button`, `GtkButton`, `Button`
/// and `gtk:Gtk.Button` all find the button; `QMainWindow` finds `<main-window>`.)
pub fn find_entities(pack: &Pack, q: &str) -> Vec<usize> {
    type Key = fn(&crate::pack::DirEntry, &str) -> bool;
    let classes: [Key; 5] = [
        |d, q| d.tags.iter().any(|t| t.eq_ignore_ascii_case(q)),
        |d, q| d.id.eq_ignore_ascii_case(q),
        |d, q| {
            d.qualified
                .as_deref()
                .is_some_and(|x| x.eq_ignore_ascii_case(q))
        },
        |d, q| d.native_name.eq_ignore_ascii_case(q),
        |d, q| d.name.eq_ignore_ascii_case(q),
    ];
    for f in classes {
        let v: Vec<usize> = pack
            .header
            .entities
            .iter()
            .enumerate()
            .filter(|(_, d)| f(d, q))
            .map(|(i, _)| i)
            .collect();
        if !v.is_empty() {
            return v;
        }
    }
    Vec::new()
}

/// A parsed member selector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberQuery {
    pub kind: Option<MemberKind>,
    pub name: String,
}

impl MemberQuery {
    pub fn parse(s: &str) -> MemberQuery {
        if let Some((k, n)) = s.split_once(':')
            && let Some(kind) = MemberKind::parse(k)
            && !n.is_empty()
        {
            return MemberQuery {
                kind: Some(kind),
                name: n.to_string(),
            };
        }
        MemberQuery {
            kind: None,
            name: s.to_string(),
        }
    }
}

/// Ways to split an address into entity + member, longest entity first. `.` separates members;
/// `::` does too when the whole string is not an entity (`QMainWindow::setCentralWidget`), but
/// never inside a member's `kind:` prefix.
pub fn splits(addr: &str) -> Vec<(String, Option<MemberQuery>)> {
    let mut out = vec![(addr.to_string(), None)];
    let mut cuts: Vec<(usize, usize)> = addr.match_indices('.').map(|(i, _)| (i, 1)).collect();
    cuts.extend(addr.match_indices("::").map(|(i, _)| (i, 2)));
    cuts.sort_by_key(|c| std::cmp::Reverse(c.0));
    for (i, w) in cuts {
        let (e, m) = (&addr[..i], &addr[i + w..]);
        if e.is_empty() || m.is_empty() {
            continue;
        }
        out.push((e.to_string(), Some(MemberQuery::parse(m))));
    }
    out
}

fn kinds_for(q: &MemberQuery) -> Vec<MemberKind> {
    match q.kind {
        Some(k) => vec![k],
        None => ALL_KINDS.to_vec(),
    }
}

/// The slots that apply to `chain[0]`: its own, else the nearest ancestor's (a kind without an
/// annotated child model inherits its base's, as `<application-window>` does `<window>`'s).
pub fn slot_owner(pack: &Pack, chain: &[usize]) -> Option<usize> {
    chain.iter().copied().find(|&i| {
        pack.entity(i)
            .children
            .as_ref()
            .is_some_and(|c| !c.slots.is_empty())
    })
}

fn hits_on<'a>(e: &'a Entity, kind: MemberKind, name: &str, loose: bool) -> Option<Member<'a>> {
    match kind {
        MemberKind::Property => e
            .properties
            .iter()
            .find(|p| eq(&p.js_name, name, loose) || eq(&p.name, name, loose))
            .map(Member::Property),
        MemberKind::Event => e
            .events
            .iter()
            .find(|v| {
                eq(&v.name, name, loose)
                    || eq(&v.js_event, name, loose)
                    || v.handler_prop
                        .as_deref()
                        .is_some_and(|h| eq(h, name, loose))
                    || v.handler_aliases.iter().any(|h| eq(h, name, loose))
            })
            .map(Member::Event),
        MemberKind::Method | MemberKind::Constructor => {
            let list = if kind == MemberKind::Method {
                &e.methods
            } else {
                &e.constructors
            };
            let first = list.iter().find(|m| eq(&m.name, name, loose))?;
            let all: Vec<&Method> = list.iter().filter(|m| m.name == first.name).collect();
            Some(if kind == MemberKind::Method {
                Member::Method(all)
            } else {
                Member::Constructor(all)
            })
        }
        MemberKind::Value => e
            .values
            .iter()
            .find(|v| {
                eq(&v.name, name, loose) || v.nick.as_deref().is_some_and(|n| eq(n, name, loose))
            })
            .map(Member::Value),
        MemberKind::Slot => e
            .children
            .as_ref()?
            .slots
            .iter()
            .find(|s| eq(&s.name, name, loose))
            .map(Member::Slot),
        MemberKind::Attr => e.children.as_ref()?.slots.iter().find_map(|s| {
            s.attrs
                .iter()
                .find(|a| eq(a, name, loose))
                .map(|a| Member::Attr {
                    slot: s,
                    attr: a.as_str(),
                })
        }),
    }
}

/// All hits for `q` on the chain of `entity` (first owner per kind wins), after suppression.
pub fn find_members<'a>(pack: &'a Pack, entity: usize, q: &MemberQuery) -> Vec<Hit<'a>> {
    let chain = pack.chain(entity);
    let slots_at = slot_owner(pack, &chain);
    for loose in [false, true] {
        let mut hits = Vec::new();
        for kind in kinds_for(q) {
            let owners: Vec<usize> = match kind {
                MemberKind::Slot | MemberKind::Attr => slots_at.into_iter().collect(),
                _ => chain.clone(),
            };
            for owner in owners {
                if let Some(member) = hits_on(pack.entity(owner), kind, &q.name, loose) {
                    hits.push(Hit {
                        kind,
                        owner,
                        member,
                    });
                    break;
                }
            }
        }
        if !hits.is_empty() {
            return suppress(hits);
        }
    }
    Vec::new()
}

/// Drops candidates that are another candidate seen from a different angle, so common lookups are
/// not ambiguous:
/// - a method that is the accessor of a matching property (Qt `text()` for the `text` property,
///   GTK accessors listed in the IR);
/// - a property named like a matching slot (GTK `child`, `titlebar`): children go in the slot, and
///   the widget-typed property is the slot's native setter (DM1 marks those dangerous).
fn suppress(hits: Vec<Hit<'_>>) -> Vec<Hit<'_>> {
    let props: Vec<&Property> = hits
        .iter()
        .filter_map(|h| {
            if let Member::Property(p) = h.member {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    let slot_names: Vec<&str> = hits
        .iter()
        .filter_map(|h| {
            if let Member::Slot(s) = h.member {
                Some(s.name.as_str())
            } else {
                None
            }
        })
        .collect();
    hits.into_iter()
        .filter(|h| match &h.member {
            Member::Method(ms) => !props.iter().any(|p| {
                let n = &ms[0].name;
                n.eq_ignore_ascii_case(&p.name)
                    || p.accessors
                        .values()
                        .any(|a| a == n || a.ends_with(&format!("_{n}")))
            }),
            Member::Property(p) => !slot_names.iter().any(|s| s.eq_ignore_ascii_case(&p.name)),
            _ => true,
        })
        .collect()
}

/// Every member visible on `entity` (own and inherited, first owner wins), per kind, in page
/// order: used by cards, `--all` and did-you-mean.
pub fn visible_members<'a>(pack: &'a Pack, entity: usize, kind: MemberKind) -> Vec<Hit<'a>> {
    let chain = pack.chain(entity);
    let mut out: Vec<Hit> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let owners: Vec<usize> = match kind {
        MemberKind::Slot | MemberKind::Attr => slot_owner(pack, &chain).into_iter().collect(),
        _ => chain,
    };
    for owner in owners {
        let e = pack.entity(owner);
        match kind {
            MemberKind::Property => {
                for p in &e.properties {
                    if seen.insert(p.name.clone()) {
                        out.push(Hit {
                            kind,
                            owner,
                            member: Member::Property(p),
                        });
                    }
                }
            }
            MemberKind::Event => {
                for v in &e.events {
                    if seen.insert(v.name.clone()) {
                        out.push(Hit {
                            kind,
                            owner,
                            member: Member::Event(v),
                        });
                    }
                }
            }
            MemberKind::Method | MemberKind::Constructor => {
                let list = if kind == MemberKind::Method {
                    &e.methods
                } else {
                    &e.constructors
                };
                for m in list {
                    if seen.insert(m.name.clone()) {
                        let all: Vec<&Method> = list.iter().filter(|x| x.name == m.name).collect();
                        let member = if kind == MemberKind::Method {
                            Member::Method(all)
                        } else {
                            Member::Constructor(all)
                        };
                        out.push(Hit {
                            kind,
                            owner,
                            member,
                        });
                    }
                }
                // constructors are not inherited
                if kind == MemberKind::Constructor {
                    break;
                }
            }
            MemberKind::Value => {
                for v in &e.values {
                    out.push(Hit {
                        kind,
                        owner,
                        member: Member::Value(v),
                    });
                }
                break;
            }
            MemberKind::Slot => {
                for s in e.children.iter().flat_map(|c| &c.slots) {
                    out.push(Hit {
                        kind,
                        owner,
                        member: Member::Slot(s),
                    });
                }
            }
            MemberKind::Attr => {
                for s in e.children.iter().flat_map(|c| &c.slots) {
                    for a in &s.attrs {
                        if seen.insert(a.clone()) {
                            out.push(Hit {
                                kind,
                                owner,
                                member: Member::Attr { slot: s, attr: a },
                            });
                        }
                    }
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Fuzzy matching
// ---------------------------------------------------------------------------------------------

/// Levenshtein distance over lower-cased chars.
pub fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().flat_map(char::to_lowercase).collect();
    let b: Vec<char> = b.chars().flat_map(char::to_lowercase).collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(ca != cb))
                .min(prev[j + 1] + 1)
                .min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// A did-you-mean score for `cand` against `q` (lower is better), or None when unrelated.
pub fn near(q: &str, cand: &str) -> Option<usize> {
    let (nq, nc) = (norm(q), norm(cand));
    if nq.is_empty() || nc.is_empty() {
        return None;
    }
    if nc == nq {
        return Some(0);
    }
    let d = distance(&nq, &nc);
    let limit = (nq.chars().count() / 3).max(2);
    if d <= limit {
        return Some(d);
    }
    // substring either way, ranked after close edits
    if nq.len() >= 3 && (nc.contains(&nq) || nq.contains(&nc)) {
        return Some(limit + 1 + nc.len().abs_diff(nq.len()));
    }
    // a close word of a compound name (`titel` → `window-title`, `windowTitle`)
    if nq.len() >= 3
        && words(cand)
            .iter()
            .any(|w| w.len() >= 3 && distance(&nq, w) <= limit)
    {
        return Some(limit + 2 + nc.len().abs_diff(nq.len()));
    }
    None
}

/// The words of a kebab, snake or camel case name, lower-cased.
fn words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for c in s.chars() {
        if c == '-' || c == '_' || c == ':' || c == '.' {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            prev_lower = false;
            continue;
        }
        if c.is_uppercase() && prev_lower && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        prev_lower = c.is_lowercase() || c.is_ascii_digit();
        cur.extend(c.to_lowercase());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Best entities for a missed entity query: (score, directory index), best first.
pub fn suggest_entities(pack: &Pack, q: &str, n: usize) -> Vec<(usize, usize)> {
    let mut v: Vec<(usize, usize)> = pack
        .header
        .entities
        .iter()
        .enumerate()
        .filter_map(|(i, d)| {
            let keys = d
                .tags
                .iter()
                .map(String::as_str)
                .chain([d.native_name.as_str(), d.name.as_str()])
                .chain(d.qualified.as_deref());
            // elements first on ties
            keys.filter_map(|k| near(q, k))
                .min()
                .map(|s| (s * 2 + usize::from(d.tags.is_empty()), i))
        })
        .collect();
    v.sort();
    v.truncate(n);
    v
}

/// Best visible members for a missed member query.
pub fn suggest_members<'a>(
    pack: &'a Pack,
    entity: usize,
    q: &MemberQuery,
    n: usize,
) -> Vec<Hit<'a>> {
    let mut scored: Vec<(usize, usize, Hit)> = Vec::new();
    let mut order = 0;
    for kind in kinds_for(q) {
        for h in visible_members(pack, entity, kind) {
            let mut best = near(&q.name, h.name());
            if let Member::Property(p) = &h.member {
                best = best.into_iter().chain(near(&q.name, &p.name)).min();
            }
            if let Some(s) = best {
                scored.push((s, order, h));
            }
            order += 1;
        }
    }
    scored.sort_by_key(|(s, o, _)| (*s, *o));
    scored.into_iter().take(n).map(|(_, _, h)| h).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_query_prefixes() {
        assert_eq!(
            MemberQuery::parse("prop:title"),
            MemberQuery {
                kind: Some(MemberKind::Property),
                name: "title".into()
            }
        );
        assert_eq!(
            MemberQuery::parse("event:close-request").kind,
            Some(MemberKind::Event)
        );
        assert_eq!(MemberQuery::parse("attr:row").kind, Some(MemberKind::Attr));
        // an unknown prefix is part of the name
        assert_eq!(
            MemberQuery::parse("foo:bar"),
            MemberQuery {
                kind: None,
                name: "foo:bar".into()
            }
        );
    }

    #[test]
    fn splits_longest_entity_first() {
        let s = splits("Gtk.Button.label");
        assert_eq!(s[0].0, "Gtk.Button.label");
        assert_eq!(
            s[1],
            ("Gtk.Button".into(), Some(MemberQuery::parse("label")))
        );
        assert_eq!(
            s[2],
            ("Gtk".into(), Some(MemberQuery::parse("Button.label")))
        );
        let q = splits("QMainWindow::setCentralWidget");
        assert_eq!(
            q[1],
            (
                "QMainWindow".into(),
                Some(MemberQuery::parse("setCentralWidget"))
            )
        );
        // the kind prefix's colon is not a separator
        assert_eq!(
            splits("main-window.prop:title")[1],
            ("main-window".into(), Some(MemberQuery::parse("prop:title")))
        );
    }

    #[test]
    fn fuzzy() {
        assert_eq!(distance("buton", "button"), 1);
        assert_eq!(near("pushbutton", "push-button"), Some(0));
        assert!(near("button", "check-button").is_some());
        assert_eq!(near("xyz", "button"), None);
        assert!(near("titel", "window-title").is_some());
        assert!(near("titel", "windowTitle").is_some());
        assert_eq!(words("setCentralWidget"), ["set", "central", "widget"]);
        assert_eq!(words("can-shrink"), ["can", "shrink"]);
    }
}
