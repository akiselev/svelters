//! Stable ids of the built-in kinds and their members (W-C1, presto `compiler-and-js.md` §3.1).
//!
//! An id is assigned once, in the checked-in table [`IDS_PATH`], and never changes or comes back:
//! a new kind gets the next kind id, a new prop, signal or method the next number among its kind's
//! members of that category, and a row whose kind or member is gone stays in the table (retired).
//! Ids therefore don't follow the order of the kind tables, and compiled code can carry them
//! (`$n.setp(n, P_TEXT, v)`) across rebuilds of the backend.
//!
//! The wire id of a member is `kind id << 16 | number`, with the kind id of the kind that declares
//! it. Provider kinds (`#[svelters::widget]`) are per binary and keep positional ids: kind ids
//! `62 << 22 | (local + 1)`, member owners `0x800 | (local + 1)`. Member ids leave their top 4 bits
//! to the listen flags (`SVL_SIGNAL_FLAGS_SHIFT`), so built-in kind ids stay below `0x800`.
//!
//! A kind or member missing from the table still gets an id (after the table's, in table order)
//! so a build works, but `tests/spec.rs` fails until the table is regenerated
//! (`SVELTERS_UPDATE=1 cargo test -p svelters-egui --test spec`), which appends the rows.

use serde_json::{Map, Value as J, json};
use std::collections::{BTreeMap, HashMap};

/// Where the table lives, relative to the workspace root.
pub const IDS_PATH: &str = "metadata/semantics/egui.ids.json";

/// The table's schema.
pub const IDS_SCHEMA: &str = "svelters-ids/v0";

const IDS_JSON: &str = include_str!("../../../../metadata/semantics/egui.ids.json");

/// A member category: numbers are assigned per kind and category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cat {
    Prop,
    Signal,
    Method,
}

/// The table file: kind id -> its ids.
#[derive(Clone, Debug, Default)]
pub struct IdFile {
    pub schema: String,
    pub backend: String,
    pub kinds: BTreeMap<String, KindIds>,
}

/// One kind's row: its kind id and the numbers of its own members.
#[derive(Clone, Debug, Default)]
pub struct KindIds {
    pub id: u32,
    pub props: BTreeMap<String, u16>,
    pub signals: BTreeMap<String, u16>,
    pub methods: BTreeMap<String, u16>,
}

const CATS: [(Cat, &str); 3] = [(Cat::Prop, "props"), (Cat::Signal, "signals"), (Cat::Method, "methods")];

impl IdFile {
    /// Parses the table's JSON.
    pub fn parse(text: &str) -> Result<IdFile, String> {
        let j: J = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let str_of = |k: &str| j.get(k).and_then(J::as_str).map(str::to_string).ok_or_else(|| format!("no `{k}`"));
        let mut kinds = BTreeMap::new();
        for (name, row) in j.get("kinds").and_then(J::as_object).ok_or("no `kinds`")? {
            let id = row.get("id").and_then(J::as_u64).filter(|&id| id > 0 && id < 0x800).ok_or_else(|| format!("kind {name}: no `id` below 0x800"))? as u32;
            let mut k = KindIds { id, ..KindIds::default() };
            for (c, key) in CATS {
                for (m, n) in row.get(key).and_then(J::as_object).into_iter().flatten() {
                    let n = n.as_u64().filter(|&n| n > 0 && n < 0x8000).ok_or_else(|| format!("{name}.{m}: bad number"))?;
                    k.cat_mut(c).insert(m.clone(), n as u16);
                }
            }
            kinds.insert(name.clone(), k);
        }
        Ok(IdFile { schema: str_of("schema")?, backend: str_of("backend")?, kinds })
    }

    /// The table as JSON (kinds and members sorted by name).
    pub fn to_json(&self) -> J {
        let kinds: Map<String, J> = self
            .kinds
            .iter()
            .map(|(name, k)| {
                let mut row = Map::new();
                row.insert("id".into(), json!(k.id));
                for (c, key) in CATS {
                    row.insert(key.into(), J::Object(k.cat(c).iter().map(|(m, n)| (m.clone(), json!(n))).collect()));
                }
                (name.clone(), J::Object(row))
            })
            .collect();
        json!({"backend": self.backend, "kinds": kinds, "schema": self.schema})
    }
}

impl KindIds {
    fn cat(&self, c: Cat) -> &BTreeMap<String, u16> {
        match c {
            Cat::Prop => &self.props,
            Cat::Signal => &self.signals,
            Cat::Method => &self.methods,
        }
    }

    fn cat_mut(&mut self, c: Cat) -> &mut BTreeMap<String, u16> {
        match c {
            Cat::Prop => &mut self.props,
            Cat::Signal => &mut self.signals,
            Cat::Method => &mut self.methods,
        }
    }
}

/// The checked-in table, parsed.
pub fn checked_in() -> IdFile {
    let f = IdFile::parse(IDS_JSON).unwrap_or_else(|e| panic!("svelters-egui: {IDS_PATH}: {e}"));
    assert_eq!(f.schema, IDS_SCHEMA, "svelters-egui: {IDS_PATH}: schema {:?}", f.schema);
    f
}

/// The ids in use: the checked-in table plus the rows assigned for kinds and members it lacks.
#[derive(Clone, Debug)]
pub struct StableIds {
    file: IdFile,
    /// The rows added to [`Self::file`] (the table is stale while this is set).
    added: bool,
    /// Kind id -> kind name.
    by_id: HashMap<u32, String>,
}

impl StableIds {
    pub fn new(file: IdFile) -> StableIds {
        let by_id = file.kinds.iter().map(|(k, r)| (r.id, k.clone())).collect();
        StableIds { file, added: false, by_id }
    }

    /// The kind id of `kind`, assigning the next one if the table lacks it.
    pub fn ensure_kind(&mut self, kind: &str) -> u32 {
        if let Some(r) = self.file.kinds.get(kind) {
            return r.id;
        }
        let id = self.file.kinds.values().map(|r| r.id).max().unwrap_or(0) + 1;
        assert!(id < 0x800, "svelters-egui: built-in kind ids are below 0x800 (provider member owners start there)");
        self.file.kinds.insert(kind.to_string(), KindIds { id, ..KindIds::default() });
        self.by_id.insert(id, kind.to_string());
        self.added = true;
        id
    }

    /// The number of member `name` of `kind` in category `c`, assigning the next one if the
    /// table lacks it.
    pub fn ensure(&mut self, kind: &str, c: Cat, name: &str) -> u16 {
        self.ensure_kind(kind);
        let row = self.file.kinds.get_mut(kind).expect("kind row").cat_mut(c);
        if let Some(&n) = row.get(name) {
            return n;
        }
        let n = row.values().copied().max().unwrap_or(0) + 1;
        row.insert(name.to_string(), n);
        self.added = true;
        n
    }

    pub fn kind(&self, kind: &str) -> Option<u32> {
        self.file.kinds.get(kind).map(|r| r.id)
    }

    /// The kind named by a kind id.
    pub fn kind_name(&self, id: u32) -> Option<&str> {
        self.by_id.get(&id).map(String::as_str)
    }

    pub fn member(&self, kind: &str, c: Cat, name: &str) -> Option<u16> {
        self.file.kinds.get(kind)?.cat(c).get(name).copied()
    }

    /// Whether rows were assigned that the checked-in table lacks.
    pub fn stale(&self) -> bool {
        self.added
    }

    /// The table with every assigned row, as written to [`IDS_PATH`].
    pub fn text(&self) -> String {
        let mut s = serde_json::to_string_pretty(&self.file.to_json()).expect("ids json");
        s.push('\n');
        s
    }
}

/// FNV-1a (64-bit) of `parts`, each followed by a NUL: the fingerprints and the spec hash, which
/// the compiler compares as text and the host recomputes, so they need a fixed, simple function.
pub fn fnv(parts: &[&str]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in p.bytes().chain(std::iter::once(0)) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// A 64-bit hash as the spec writes it (16 lowercase hex digits).
pub fn hex(h: u64) -> String {
    format!("{h:016x}")
}
