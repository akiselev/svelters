//! The docs pack: a projection of one toolkit's docs IR (`docs/site/.generated/ir/<tk>.json`,
//! schema `tools/docs/ir/schema.json`), the same file the Starlight site renders.
//!
//! The structs below are both the projection and the pack's entity format: they deserialize from
//! the IR (unknown IR fields are ignored, so the IR can grow without breaking the pack) and
//! serialize into the pack. Member extensions, provenance and origin lists are dropped; the
//! builder adds what the reader needs and the IR does not carry directly: the site route of every
//! entity and the site anchor of every member (computed exactly like `docs/site/scripts/lib/render.mjs`,
//! which `tests/site_consistency.rs` checks).
//!
//! Container (`<tk>.svpack`, all integers little-endian):
//!
//! ```text
//! 0   b"SVPK"
//! 4   u16 format version (1)
//! 6   u16 reserved (0)
//! 8   u32 length of the compressed header
//! 12  header: raw deflate of the JSON `Header` (toolkit, data version, entity directory)
//! ..  entity blobs: raw deflate of each entity's JSON; the directory holds (offset, length)
//!     relative to the end of the header
//! ```
//!
//! A lookup inflates the header and only the entities it touches (an element plus its ancestors);
//! `-k` search inflates all of them.

use std::cell::OnceCell;
use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAGIC: &[u8; 4] = b"SVPK";
pub const FORMAT_VERSION: u16 = 1;
pub const HEADER_SCHEMA: &str = "svelters-docs-pack/1";
/// The IR schema the projection is written against.
pub const IR_SCHEMA: &str = "svelters-docs-ir/v0";

fn is_false(b: &bool) -> bool {
    !*b
}

// ---------------------------------------------------------------------------------------------
// Projection (IR in, pack entity out)
// ---------------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Text {
    pub md: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct TypeRef {
    pub display: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native: Option<String>,
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub nullable: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Param {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: TypeRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Availability {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// `true`, a version string, or absent.
    #[serde(default, skip_serializing_if = "deprecated_absent")]
    pub deprecated: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated_note: Option<String>,
}

fn deprecated_absent(v: &Value) -> bool {
    matches!(v, Value::Null | Value::Bool(false))
}

impl Availability {
    pub fn is_empty(&self) -> bool {
        self.since.is_none()
            && deprecated_absent(&self.deprecated)
            && self.deprecated_note.is_none()
    }
}

fn avail_absent(a: &Option<Availability>) -> bool {
    a.as_ref().is_none_or(Availability::is_empty)
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Dangerous {
    pub severity: String,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_instead: Option<String>,
    /// `false` when app code cannot reach the member today; absent means reachable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reachable: Option<bool>,
    pub source: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Access {
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub write: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub construct_only: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Property {
    pub js_name: String,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: TypeRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<Value>,
    #[serde(default)]
    pub access: Access,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub bindable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub accessors: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub annotated: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub verified: bool,
    #[serde(default, skip_serializing_if = "avail_absent")]
    pub availability: Option<Availability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dangerous: Option<Dangerous>,
    /// Site anchor (`property-<slug>`), filled by the builder.
    #[serde(default)]
    pub anchor: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Event {
    pub name: String,
    pub js_event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handler_prop: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub handler_aliases: Vec<String>,
    #[serde(default)]
    pub params: Vec<Param>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<TypeRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handler_result: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub sync_required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub annotated: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub verified: bool,
    #[serde(default, skip_serializing_if = "avail_absent")]
    pub availability: Option<Availability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dangerous: Option<Dangerous>,
    #[serde(default)]
    pub anchor: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Method {
    pub name: String,
    pub signature: String,
    #[serde(default)]
    pub params: Vec<Param>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<TypeRef>,
    #[serde(default, rename = "static", skip_serializing_if = "is_false")]
    pub is_static: bool,
    #[serde(default, rename = "virtual", skip_serializing_if = "is_false")]
    pub is_virtual: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsupported_reason: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub annotated: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub verified: bool,
    #[serde(default, skip_serializing_if = "avail_absent")]
    pub availability: Option<Availability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dangerous: Option<Dangerous>,
    #[serde(default)]
    pub anchor: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct EnumValue {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nick: Option<String>,
    /// Integer or string, as in the IR.
    pub value: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<Text>,
    #[serde(default)]
    pub anchor: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Slot {
    pub name: String,
    pub cardinality: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accepts: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub default: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ops: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ownership: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_remove: Option<String>,
    /// Child attributes this slot gives its children (`parent:<attr>`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attrs: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub annotated: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub verified: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Children {
    #[serde(default)]
    pub policy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_slot: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<Slot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Example {
    pub title: String,
    pub lang: String,
    pub code: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub generated: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub tested: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Lifetime {
    pub class: String,
    #[serde(default)]
    pub origin: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Status {
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub tested: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_facts: Option<(u32, u32)>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Upstream {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Entity {
    pub id: String,
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualified: Option<String>,
    pub native_name: String,
    #[serde(default, rename = "abstract", skip_serializing_if = "is_false")]
    pub is_abstract: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<Upstream>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bases: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interfaces: Vec<String>,
    #[serde(default, skip_serializing_if = "avail_absent")]
    pub availability: Option<Availability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifetime: Option<Lifetime>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<Property>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<Event>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<Method>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constructors: Vec<Method>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<EnumValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Children>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<Example>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub see_also: Vec<String>,
    /// Entity-scope toolkit specifics (`extensions.<toolkit>`), rendered through the header's
    /// `extension_schemas`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, Value>,
    /// Site route (`/api/<tk>/<tag>/` or `/api/<tk>/types/<slug>/`), filled by the builder.
    #[serde(default)]
    pub route: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct License {
    #[serde(default)]
    pub spdx: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redistribution: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Toolkit {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub doc_license: License,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_policy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_docs: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
}

#[derive(Deserialize)]
struct IrDoc {
    schema: String,
    toolkit: Toolkit,
    entities: Vec<Entity>,
    #[serde(default)]
    extension_schemas: BTreeMap<String, Value>,
}

/// One row of the pack's entity directory: enough to list, match and route without inflating
/// the entity.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct DirEntry {
    pub id: String,
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualified: Option<String>,
    pub native_name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub tier: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub tested: bool,
    /// First paragraph of the summary, as plain text.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub brief: String,
    pub route: String,
    pub off: u32,
    pub len: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Header {
    pub schema: String,
    pub ir_schema: String,
    /// `<toolkit version>+<16 hex digits of FNV-1a 64 over the IR bytes>`.
    pub data_version: String,
    pub built_by: String,
    pub toolkit: Toolkit,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extension_schemas: BTreeMap<String, Value>,
    pub entities: Vec<DirEntry>,
}

// ---------------------------------------------------------------------------------------------
// Site route / anchor rules (mirror docs/site/scripts/lib/render.mjs)
// ---------------------------------------------------------------------------------------------

/// `slugify` from render.mjs: lower-case, runs of non-alphanumerics become one `-`, trimmed.
pub fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut dash = false;
    for c in s.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if dash && !out.is_empty() {
                out.push('-');
            }
            dash = false;
            out.push(c);
        } else {
            dash = true;
        }
    }
    out
}

/// `memberAnchor` from render.mjs: `<kind>-<slug>`, then `-2`, `-3`, … on collisions.
fn member_anchor(kind: &str, name: &str, used: &mut HashSet<String>) -> String {
    let base = format!("{kind}-{}", slugify(name));
    let mut a = base.clone();
    let mut n = 1;
    while used.contains(&a) {
        n += 1;
        a = format!("{base}-{n}");
    }
    used.insert(a.clone());
    a
}

/// Assigns site routes and member anchors. Order matters and follows `renderEntity`: values,
/// properties, events, constructors, methods share one `used` set per page.
fn assign_routes(tk: &str, entities: &mut [Entity]) {
    let mut seen = HashSet::new();
    for e in entities.iter_mut() {
        e.route = if let Some(tag) = e.tags.first() {
            format!("/api/{tk}/{tag}/")
        } else {
            let base = if e.native_name.is_empty() {
                &e.name
            } else {
                &e.native_name
            };
            let mut slug = slugify(base);
            while seen.contains(&slug) {
                slug.push_str("-x");
            }
            seen.insert(slug.clone());
            format!("/api/{tk}/types/{slug}/")
        };
        let mut used = HashSet::new();
        for v in &mut e.values {
            v.anchor = member_anchor("value", &v.name, &mut used);
        }
        for p in &mut e.properties {
            p.anchor = member_anchor("property", &p.name, &mut used);
        }
        for ev in &mut e.events {
            ev.anchor = member_anchor("event", &ev.name, &mut used);
        }
        for c in &mut e.constructors {
            c.anchor = member_anchor("constructor", &c.name, &mut used);
        }
        for m in &mut e.methods {
            m.anchor = member_anchor("method", &m.name, &mut used);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------------------------

/// FNV-1a 64: a dependency-free content hash for the data version (not a security property).
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// The first paragraph of a markdown text block as one line of plain text.
pub fn brief_plain(md: &str) -> String {
    let para = md
        .split("\n\n")
        .find(|p| !p.trim().is_empty())
        .unwrap_or("");
    let text = crate::text::plain(para);
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Projects one IR document (the bytes of `docs/site/.generated/ir/<tk>.json`) into a pack.
pub fn build(ir_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let ir: IrDoc =
        serde_json::from_slice(ir_bytes).map_err(|e| format!("IR does not parse: {e}"))?;
    if ir.schema != IR_SCHEMA {
        return Err(format!(
            "IR schema is {:?}, this svelters understands {IR_SCHEMA:?}",
            ir.schema
        ));
    }
    let tk = ir.toolkit.id.clone();
    let mut entities = ir.entities;
    assign_routes(&tk, &mut entities);

    let mut blobs = Vec::new();
    let mut dir = Vec::with_capacity(entities.len());
    for e in &entities {
        let json = serde_json::to_vec(e).map_err(|err| err.to_string())?;
        let z = miniz_oxide::deflate::compress_to_vec(&json, 9);
        dir.push(DirEntry {
            id: e.id.clone(),
            kind: e.kind.clone(),
            name: e.name.clone(),
            qualified: e.qualified.clone(),
            native_name: e.native_name.clone(),
            tags: e.tags.clone(),
            tier: e.status.tier.clone(),
            tested: e.status.tested,
            brief: e
                .summary
                .as_ref()
                .map(|s| brief_plain(&s.md))
                .unwrap_or_default(),
            route: e.route.clone(),
            off: u32::try_from(blobs.len()).map_err(|_| "pack too large")?,
            len: u32::try_from(z.len()).map_err(|_| "entity too large")?,
        });
        blobs.extend_from_slice(&z);
    }
    let header = Header {
        schema: HEADER_SCHEMA.into(),
        ir_schema: ir.schema,
        data_version: format!("{}+{:016x}", ir.toolkit.version, fnv1a64(ir_bytes)),
        built_by: format!("svelters-cli {}", env!("CARGO_PKG_VERSION")),
        toolkit: ir.toolkit,
        extension_schemas: ir.extension_schemas,
        entities: dir,
    };
    let hjson = serde_json::to_vec(&header).map_err(|e| e.to_string())?;
    let hz = miniz_oxide::deflate::compress_to_vec(&hjson, 9);
    let mut out = Vec::with_capacity(12 + hz.len() + blobs.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(
        &u32::try_from(hz.len())
            .map_err(|_| "header too large")?
            .to_le_bytes(),
    );
    out.extend_from_slice(&hz);
    out.extend_from_slice(&blobs);
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------------------------

/// Where a pack came from (printed by `svelters docs` and in `--json`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// `--pack <file>` or `SVELTERS_DOCS_PACK`.
    File(String),
    /// Built in memory from an IR file (`--ir`).
    Ir(String),
    /// The repo's `docs/site/.generated/pack/<tk>.svpack`, rebuilt from the IR when stale.
    Repo(String),
    /// Next to the executable.
    Installed(String),
    /// Compiled into the binary (`SVELTERS_DOCS_EMBED_DIR` at build time).
    Embedded,
}

impl Source {
    pub fn describe(&self) -> String {
        match self {
            Source::File(p) => format!("file {p}"),
            Source::Ir(p) => format!("built from IR {p}"),
            Source::Repo(p) => format!("repo {p}"),
            Source::Installed(p) => format!("installed {p}"),
            Source::Embedded => "embedded in the binary".into(),
        }
    }
}

pub struct Pack {
    pub header: Header,
    pub source: Source,
    bytes: std::borrow::Cow<'static, [u8]>,
    body: usize,
    cache: Vec<OnceCell<Entity>>,
}

const MAX_INFLATE: usize = 64 << 20;

fn inflate(z: &[u8]) -> Result<Vec<u8>, String> {
    miniz_oxide::inflate::decompress_to_vec_with_limit(z, MAX_INFLATE)
        .map_err(|e| format!("corrupt docs pack ({e:?})"))
}

impl Pack {
    pub fn open(bytes: std::borrow::Cow<'static, [u8]>, source: Source) -> Result<Pack, String> {
        if bytes.len() < 12 || &bytes[..4] != MAGIC {
            return Err("not a svelters docs pack (bad magic)".into());
        }
        let ver = u16::from_le_bytes([bytes[4], bytes[5]]);
        if ver != FORMAT_VERSION {
            return Err(format!(
                "docs pack format {ver}, this svelters reads format {FORMAT_VERSION}; rebuild the pack"
            ));
        }
        let hlen = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        let hz = bytes
            .get(12..12 + hlen)
            .ok_or("truncated docs pack header")?;
        let header: Header = serde_json::from_slice(&inflate(hz)?)
            .map_err(|e| format!("corrupt docs pack header: {e}"))?;
        if header.schema != HEADER_SCHEMA {
            return Err(format!(
                "docs pack schema {:?}, expected {HEADER_SCHEMA:?}",
                header.schema
            ));
        }
        let cache = header.entities.iter().map(|_| OnceCell::new()).collect();
        Ok(Pack {
            header,
            source,
            bytes,
            body: 12 + hlen,
            cache,
        })
    }

    pub fn from_ir(ir_bytes: &[u8], source: Source) -> Result<Pack, String> {
        Pack::open(std::borrow::Cow::Owned(build(ir_bytes)?), source)
    }

    pub fn toolkit(&self) -> &str {
        &self.header.toolkit.id
    }

    pub fn len(&self) -> usize {
        self.header.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.header.entities.is_empty()
    }

    /// The entity at directory index `i`, inflated on first use.
    pub fn entity(&self, i: usize) -> &Entity {
        self.cache[i].get_or_init(|| {
            let d = &self.header.entities[i];
            let start = self.body + d.off as usize;
            let z = &self.bytes[start..start + d.len as usize];
            // A pack is validated when it is built; a corrupt blob is a bug or a damaged file.
            let json = inflate(z).unwrap_or_else(|e| panic!("{e} at entity {}", d.id));
            serde_json::from_slice(&json)
                .unwrap_or_else(|e| panic!("corrupt docs pack entity {}: {e}", d.id))
        })
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.header.entities.iter().position(|d| d.id == id)
    }

    /// The entity followed by its documented ancestors, in the site's order: bases (nearest
    /// first), then interfaces, without duplicates. Ancestors missing from the pack are skipped.
    pub fn chain(&self, i: usize) -> Vec<usize> {
        let e = self.entity(i);
        let mut out = vec![i];
        for id in e.bases.iter().chain(e.interfaces.iter()) {
            if let Some(j) = self.index_of(id)
                && !out.contains(&j)
            {
                out.push(j);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_matches_render_mjs() {
        assert_eq!(slugify("can-shrink"), "can-shrink");
        assert_eq!(slugify("QMainWindow"), "qmainwindow");
        assert_eq!(slugify("Qt::AlignmentFlag"), "qt-alignmentflag");
        assert_eq!(slugify("--a__b--"), "a-b");
        assert_eq!(slugify("set_child"), "set-child");
    }

    #[test]
    fn anchors_dedupe_like_the_site() {
        let mut used = HashSet::new();
        assert_eq!(
            member_anchor("method", "resize", &mut used),
            "method-resize"
        );
        assert_eq!(
            member_anchor("method", "resize", &mut used),
            "method-resize-2"
        );
        assert_eq!(
            member_anchor("method", "resize", &mut used),
            "method-resize-3"
        );
        assert_eq!(
            member_anchor("property", "resize", &mut used),
            "property-resize"
        );
    }

    #[test]
    fn fnv_is_stable() {
        assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
    }
}
