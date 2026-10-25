//! Stable wire ids (compiler-and-js §2, §3.1): the `svl_id`s presto.json publishes and the ABI speaks
//! for kinds, props, signals, methods and slots.
//!
//! Ids come from a checked-in, append-only table (`ids.txt` next to this crate's `Cargo.toml`): one
//! row per member, `<id> <what> <Kind>[.<member>]`, keyed by the kind that declares the member (an
//! inherited prop has its base kind's id on every kind). A new member gets the next free id when the
//! table is regenerated (`SVELTERS_UPDATE=1 cargo test -p svelters-presto --test spec`); nothing is
//! ever renumbered, so adding a row moves no other id (egui's positional ids do).
//!
//! Each id has a [`fingerprint`] (a hash of kind name, member name and type) and the spec has a
//! [`Ids::spec_hash`] over all of them, which the host checks against compiled code at load.

use crate::shim::{KindRegistry, KindSpec, ValueType};
use std::collections::{BTreeMap, HashMap};
use svelters_abi::svl_id;

/// The checked-in table.
pub const TABLE: &str = include_str!("../ids.txt");
/// Where it lives, relative to the workspace root.
pub const TABLE_PATH: &str = "crates/svelters-presto/ids.txt";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum What {
    Kind,
    Prop,
    Signal,
    Method,
    Slot,
}

impl What {
    pub fn name(self) -> &'static str {
        match self {
            What::Kind => "kind",
            What::Prop => "prop",
            What::Signal => "signal",
            What::Method => "method",
            What::Slot => "slot",
        }
    }

    fn parse(s: &str) -> Option<What> {
        Some(match s {
            "kind" => What::Kind,
            "prop" => What::Prop,
            "signal" => What::Signal,
            "method" => What::Method,
            "slot" => What::Slot,
            _ => return None,
        })
    }
}

/// A row's key: the declaring kind and, for a member, its name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key {
    pub what: What,
    pub kind: String,
    pub member: Option<String>,
}

impl Key {
    pub fn kind(kind: &str) -> Key {
        Key { what: What::Kind, kind: kind.to_string(), member: None }
    }

    pub fn member(what: What, kind: &str, member: &str) -> Key {
        Key { what, kind: kind.to_string(), member: Some(member.to_string()) }
    }

    fn line(&self, id: svl_id) -> String {
        match &self.member {
            None => format!("{id} {} {}", self.what.name(), self.kind),
            Some(m) => format!("{id} {} {}.{m}", self.what.name(), self.kind),
        }
    }
}

/// FNV-1a, 64 bits: stable across platforms and Rust versions (unlike `DefaultHasher`).
pub fn fnv(parts: &[&str]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in p.bytes().chain(std::iter::once(0)) {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// The fingerprint of a member: hash of kind name, member name and type (`""` for a kind or slot).
pub fn fingerprint(kind: &str, member: &str, ty: &str) -> u64 {
    fnv(&[kind, member, ty])
}

/// The type text a fingerprint hashes for a value type (enums include their names, so adding a
/// value changes it).
pub fn type_text(ty: ValueType) -> String {
    match ty {
        ValueType::Enum(names) => format!("enum({})", names.join("|")),
        other => match other.of() {
            Some(of) => format!("list<{of}>"),
            None => other.name().to_string(),
        },
    }
}

/// The parsed table, both ways.
#[derive(Clone, Debug, Default)]
pub struct Ids {
    by_key: HashMap<Key, svl_id>,
    by_id: BTreeMap<svl_id, Key>,
}

impl Ids {
    /// Parses table text; `#` starts a comment line.
    pub fn parse(text: &str) -> Result<Ids, String> {
        let mut ids = Ids::default();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let bad = || format!("{TABLE_PATH}:{}: expected `<id> <kind|prop|signal|method|slot> <Kind>[.<member>]`: {line}", n + 1);
            let mut w = line.split_whitespace();
            let (Some(id), Some(what), Some(name), None) = (w.next(), w.next(), w.next(), w.next()) else { return Err(bad()) };
            let id: svl_id = id.parse().map_err(|_| bad())?;
            let what = What::parse(what).ok_or_else(bad)?;
            let key = match (what, name.split_once('.')) {
                (What::Kind, None) => Key::kind(name),
                (What::Kind, Some(_)) | (_, None) => return Err(bad()),
                (_, Some((k, m))) => Key::member(what, k, m),
            };
            if id == 0 || ids.by_id.contains_key(&id) {
                return Err(format!("{TABLE_PATH}:{}: id {id} is 0 or used twice", n + 1));
            }
            if ids.by_key.insert(key.clone(), id).is_some() {
                return Err(format!("{TABLE_PATH}:{}: {name} has two ids", n + 1));
            }
            ids.by_id.insert(id, key);
        }
        Ok(ids)
    }

    /// The checked-in table (panics on a malformed table: `tests/spec.rs` reports it first).
    pub fn builtin() -> &'static Ids {
        static IDS: std::sync::OnceLock<Ids> = std::sync::OnceLock::new();
        IDS.get_or_init(|| Ids::parse(TABLE).unwrap_or_else(|e| panic!("svelters-presto: {e}")))
    }

    pub fn get(&self, key: &Key) -> Option<svl_id> {
        self.by_key.get(key).copied()
    }

    pub fn key(&self, id: svl_id) -> Option<&Key> {
        self.by_id.get(&id)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Every key `registry` needs, in a deterministic order: kinds (bases first, as registered), then
    /// each kind's own props, signals, methods and slots.
    pub fn keys_of(registry: &KindRegistry) -> Vec<Key> {
        let mut keys = Vec::new();
        for (_, k) in registry.kinds() {
            keys.extend(own_keys(k));
        }
        keys
    }

    /// The keys `registry` needs that the table lacks.
    pub fn missing(&self, registry: &KindRegistry) -> Vec<Key> {
        Ids::keys_of(registry).into_iter().filter(|k| !self.by_key.contains_key(k)).collect()
    }

    /// The table text with rows for `missing` appended (new ids after the highest one).
    pub fn appended(&self, text: &str, missing: &[Key]) -> String {
        let mut next = self.by_id.keys().next_back().copied().unwrap_or(0) + 1;
        let mut out = text.trim_end().to_string();
        if !out.is_empty() {
            out.push('\n');
        }
        for k in missing {
            out.push_str(&k.line(next));
            out.push('\n');
            next += 1;
        }
        out
    }

    /// A hash of every (id, fingerprint) of `registry`'s members: the spec hash the host checks.
    pub fn spec_hash(&self, registry: &KindRegistry) -> u64 {
        let mut rows: Vec<String> = Vec::new();
        for (_, k) in registry.kinds() {
            for (key, fp) in own_fingerprints(k) {
                if let Some(id) = self.get(&key) {
                    rows.push(format!("{id}:{fp:016x}"));
                }
            }
        }
        rows.sort();
        let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
        fnv(&refs)
    }
}

/// The keys of the members `k` declares itself.
pub fn own_keys(k: &KindSpec) -> Vec<Key> {
    own_fingerprints(k).into_iter().map(|(k, _)| k).collect()
}

/// The keys and fingerprints of the members `k` declares itself.
pub fn own_fingerprints(k: &KindSpec) -> Vec<(Key, u64)> {
    let mut v = vec![(Key::kind(k.name), fingerprint(k.name, "", k.tag.unwrap_or("")))];
    for p in k.props {
        v.push((Key::member(What::Prop, k.name, p.name), fingerprint(k.name, p.name, &type_text(p.ty))));
    }
    for s in k.signals {
        let args: Vec<String> = s.args.iter().map(|(n, t)| format!("{n}:{}", type_text(*t))).collect();
        v.push((Key::member(What::Signal, k.name, s.name), fingerprint(k.name, s.name, &args.join(","))));
    }
    for m in k.methods {
        let params: Vec<String> = m.params.iter().map(|(n, t)| format!("{n}:{}", type_text(*t))).collect();
        let ret = m.ret.map(type_text).unwrap_or_default();
        v.push((Key::member(What::Method, k.name, m.name), fingerprint(k.name, m.name, &format!("({})->{ret}", params.join(",")))));
    }
    for s in k.slots {
        v.push((Key::member(What::Slot, k.name, s.name), fingerprint(k.name, s.name, "")));
    }
    v
}
