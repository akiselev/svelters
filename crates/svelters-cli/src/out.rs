//! The output model of `svelters docs`. `--json` serializes it as is (schema `svelters-docs/1`,
//! documented in `plans/milestones/CL0-cli-docs.md`); the markdown renderer (`md.rs`) reads the
//! same values, so the two formats cannot disagree.
//!
//! Stability rules for `svelters-docs/1`: fields are only added, never renamed or retyped; absent
//! optional fields are omitted rather than `null`; arrays are always present (possibly empty).

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::pack::{Dangerous, Example, Pack, TypeRef};
use crate::resolve::{self, Hit, Member, MemberKind};
use crate::text;

pub const SCHEMA: &str = "svelters-docs/1";

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Serialize, Clone, Debug)]
pub struct TypeOut {
    pub display: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native: Option<String>,
    /// Entity id the type documents (`gtk:Gtk.Widget`), when it is in the docs.
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub nullable: bool,
}

impl From<&TypeRef> for TypeOut {
    fn from(t: &TypeRef) -> Self {
        TypeOut {
            display: t.display.clone(),
            native: t.native.clone(),
            ref_: t.ref_.clone(),
            nullable: t.nullable,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct ParamOut {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: TypeOut,
}

#[derive(Serialize, Clone, Debug)]
pub struct DangerOut {
    /// `corrupts-tree` | `owned-by-backend` | `crashes` | `other`.
    pub severity: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_instead: Option<String>,
    /// False when app code cannot reach the member today.
    pub reachable: bool,
    /// Where the marking is declared (`metadata/dangerous/<tk>.yaml#<rule>` or an override).
    pub source: String,
}

impl From<&Dangerous> for DangerOut {
    fn from(d: &Dangerous) -> Self {
        DangerOut {
            severity: d.severity.clone(),
            reason: text::clean_md(&d.reason),
            use_instead: d.use_instead.as_deref().map(text::clean_md),
            reachable: d.reachable != Some(false),
            source: d.source.clone(),
        }
    }
}

/// Fields every member carries.
#[derive(Serialize, Clone, Debug)]
pub struct MemberBase {
    /// `property` | `event` | `method` | `constructor` | `value` | `slot` | `attr`.
    pub kind: String,
    /// The svelters name: kebab-case prop/event/attr name, or the native method/value/slot name.
    pub name: String,
    /// Canonical address, accepted by `svelters docs`: `gtk button.prop:label`.
    pub address: String,
    /// Entity id that declares it (differs from the card's entity for inherited members).
    pub declared_in: String,
    /// Site path with anchor (`/api/gtk/button/#property-label`); empty for slots and attrs.
    pub site: String,
    /// Hand/agent-annotated (`metadata/overrides`).
    pub annotated: bool,
    /// Covered by a passing generated verification test (`metadata/verified`).
    pub verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// `true`, or the version it was deprecated in (as a string).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<Value>,
    /// Documentation markdown (links reduced to text). Omitted with `--brief`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dangerous: Option<DangerOut>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum MemberOut {
    Property {
        #[serde(flatten)]
        base: MemberBase,
        /// Native property name (`windowTitle`, `can-shrink`).
        native: String,
        #[serde(rename = "type")]
        ty: TypeOut,
        /// `rw` | `r` | `w` | `construct-only`.
        access: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        default: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        notify: Option<String>,
        bindable: bool,
        #[serde(skip_serializing_if = "BTreeMap::is_empty")]
        accessors: BTreeMap<String, String>,
    },
    Event {
        #[serde(flatten)]
        base: MemberBase,
        /// Native signal name.
        native: String,
        /// The handler attribute (`onclicked`).
        #[serde(skip_serializing_if = "Option::is_none")]
        attr: Option<String>,
        aliases: Vec<String>,
        params: Vec<ParamOut>,
        #[serde(skip_serializing_if = "Option::is_none")]
        returns: Option<TypeOut>,
        /// The handler runs synchronously (it can return a result / preventDefault).
        sync: bool,
        /// `none` | `bool_stop` | `value`.
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        phase: Option<String>,
        flags: Vec<String>,
    },
    Method {
        #[serde(flatten)]
        base: MemberBase,
        /// One per overload.
        signatures: Vec<String>,
        params: Vec<ParamOut>,
        #[serde(skip_serializing_if = "Option::is_none")]
        returns: Option<TypeOut>,
        #[serde(rename = "static")]
        is_static: bool,
        #[serde(rename = "virtual")]
        is_virtual: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        unsupported: Option<String>,
    },
    Value {
        #[serde(flatten)]
        base: MemberBase,
        #[serde(skip_serializing_if = "Option::is_none")]
        nick: Option<String>,
        value: Value,
    },
    Slot {
        #[serde(flatten)]
        base: MemberBase,
        default: bool,
        /// `one` | `many`.
        cardinality: String,
        accepts: Vec<String>,
        /// Child attributes this slot gives its children (`parent:<attr>`).
        attrs: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        order: Option<String>,
        ops: BTreeMap<String, String>,
        ownership: BTreeMap<String, String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        on_remove: Option<String>,
    },
    Attr {
        #[serde(flatten)]
        base: MemberBase,
        /// Written on the child as `parent:<name>`.
        attribute: String,
        /// The slot that gives it.
        slot: String,
    },
}

impl MemberOut {
    pub fn base(&self) -> &MemberBase {
        match self {
            MemberOut::Property { base, .. }
            | MemberOut::Event { base, .. }
            | MemberOut::Method { base, .. }
            | MemberOut::Value { base, .. }
            | MemberOut::Slot { base, .. }
            | MemberOut::Attr { base, .. } => base,
        }
    }
}

fn site(pack: &Pack, owner: usize, anchor: &str) -> String {
    let r = &pack.header.entities[owner].route;
    if anchor.is_empty() {
        r.clone()
    } else {
        format!("{r}#{anchor}")
    }
}

fn doc(t: &Option<crate::pack::Text>, brief: bool) -> Option<String> {
    if brief {
        return None;
    }
    t.as_ref()
        .map(|t| text::clean_md(&t.md))
        .filter(|s| !s.is_empty())
}

fn access(a: &crate::pack::Access) -> &'static str {
    if a.construct_only {
        "construct-only"
    } else if a.read && a.write {
        "rw"
    } else if a.write {
        "w"
    } else {
        "r"
    }
}

/// Builds the output record for a hit. `entity` is the card's entity (for the address).
pub fn member_out(pack: &Pack, entity: usize, h: &Hit, brief: bool) -> MemberOut {
    let declared_in = pack.header.entities[h.owner].id.clone();
    let base = |anchor: &str,
                annotated: bool,
                verified: bool,
                av: &Option<crate::pack::Availability>,
                d: &Option<crate::pack::Text>,
                dg: &Option<Dangerous>| MemberBase {
        kind: h.kind.word().into(),
        name: h.name().into(),
        address: resolve::member_address(pack, entity, h),
        declared_in: declared_in.clone(),
        site: site(pack, h.owner, anchor),
        annotated,
        verified,
        since: av.as_ref().and_then(|a| a.since.clone()),
        deprecated: av
            .as_ref()
            .map(|a| a.deprecated.clone())
            .filter(|v| !matches!(v, Value::Null | Value::Bool(false))),
        doc: doc(d, brief),
        dangerous: dg.as_ref().map(DangerOut::from),
    };
    match &h.member {
        Member::Property(p) => MemberOut::Property {
            base: base(
                &p.anchor,
                p.annotated,
                p.verified,
                &p.availability,
                &p.doc,
                &p.dangerous,
            ),
            native: p.name.clone(),
            ty: (&p.ty).into(),
            access: access(&p.access).into(),
            default: p.default.clone(),
            range: p.range.clone().filter(|r| !r.is_null()),
            notify: p.notify.clone(),
            bindable: p.bindable,
            accessors: p.accessors.clone(),
        },
        Member::Event(v) => MemberOut::Event {
            base: base(
                &v.anchor,
                v.annotated,
                v.verified,
                &v.availability,
                &v.doc,
                &v.dangerous,
            ),
            native: v.name.clone(),
            attr: v.handler_prop.clone(),
            aliases: v.handler_aliases.clone(),
            params: v
                .params
                .iter()
                .map(|p| ParamOut {
                    name: p.name.clone(),
                    ty: (&p.ty).into(),
                })
                .collect(),
            returns: v.returns.as_ref().map(TypeOut::from),
            sync: v.sync_required,
            result: v.handler_result.clone(),
            phase: v.phase.clone(),
            flags: v.flags.clone(),
        },
        Member::Method(ms) | Member::Constructor(ms) => {
            let m = ms[0];
            // the first overload with documentation or a danger marking describes the group
            let d = ms.iter().find_map(|m| m.doc.clone());
            let dg = ms.iter().find_map(|m| m.dangerous.clone());
            let mut b = base(
                &m.anchor,
                ms.iter().any(|m| m.annotated),
                ms.iter().any(|m| m.verified),
                &m.availability,
                &d,
                &dg,
            );
            b.kind = h.kind.word().into();
            MemberOut::Method {
                base: b,
                signatures: ms.iter().map(|m| m.signature.clone()).collect(),
                params: m
                    .params
                    .iter()
                    .map(|p| ParamOut {
                        name: p.name.clone(),
                        ty: (&p.ty).into(),
                    })
                    .collect(),
                returns: m.returns.as_ref().map(TypeOut::from),
                is_static: m.is_static,
                is_virtual: m.is_virtual,
                unsupported: m.unsupported_reason.clone(),
            }
        }
        Member::Value(v) => MemberOut::Value {
            base: base(&v.anchor, false, false, &None, &v.doc, &None),
            nick: v.nick.clone(),
            value: v.value.clone(),
        },
        Member::Slot(s) => MemberOut::Slot {
            base: base("", s.annotated, s.verified, &None, &None, &None),
            default: s.default,
            cardinality: s.cardinality.clone(),
            accepts: s.accepts.clone(),
            attrs: s.attrs.clone(),
            order: s.order.clone(),
            ops: s.ops.clone(),
            ownership: s.ownership.clone(),
            on_remove: s.on_remove.clone(),
        },
        Member::Attr { slot, attr } => MemberOut::Attr {
            base: base("", slot.annotated, slot.verified, &None, &None, &None),
            attribute: format!("parent:{attr}"),
            slot: slot.name.clone(),
        },
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct EntityOut {
    pub id: String,
    /// `gtk button` (the tag, else the qualified/native name).
    pub address: String,
    /// `class` | `interface` | `enum` | `flags` | `struct` | `boxed`.
    pub kind: String,
    pub name: String,
    pub native: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualified: Option<String>,
    /// Svelte element tags (empty for types that are not elements).
    pub tags: Vec<String>,
    /// `annotated` | `ingested`.
    pub tier: String,
    pub tested: bool,
    /// `[verified, total]` annotated facts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_facts: Option<(u32, u32)>,
    #[serde(rename = "abstract")]
    pub is_abstract: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub site: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    /// Base entity ids, nearest first (may name undocumented types).
    pub bases: Vec<String>,
    pub interfaces: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifetime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// Toolkit specifics (`{"qt": {"header": …, "module": …}}`), labels in the toolkit listing.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, Value>,
    pub see_also: Vec<String>,
}

pub fn entity_out(pack: &Pack, i: usize, brief: bool) -> EntityOut {
    let e = pack.entity(i);
    EntityOut {
        id: e.id.clone(),
        address: resolve::entity_address(pack, i),
        kind: e.kind.clone(),
        name: e.name.clone(),
        native: e.native_name.clone(),
        qualified: e.qualified.clone(),
        tags: e.tags.clone(),
        tier: e.status.tier.clone(),
        tested: e.status.tested,
        verified_facts: e.status.verified_facts,
        is_abstract: e.is_abstract,
        summary: e.summary.as_ref().map(|s| {
            if brief {
                text::first_paragraph(&s.md)
            } else {
                text::clean_md(&s.md)
            }
        }),
        description: doc(&e.description, brief),
        site: e.route.clone(),
        upstream: e
            .upstream
            .as_ref()
            .map(|u| u.url.clone())
            .filter(|u| !u.is_empty()),
        bases: e.bases.clone(),
        interfaces: e.interfaces.clone(),
        lifetime: e
            .lifetime
            .as_ref()
            .map(|l| l.class.clone())
            .filter(|c| c != "unknown"),
        since: e.availability.as_ref().and_then(|a| a.since.clone()),
        extensions: if brief {
            BTreeMap::new()
        } else {
            e.extensions.clone()
        },
        see_also: e.see_also.clone(),
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct InheritedOut {
    /// Entity id of the ancestor.
    pub from: String,
    pub name: String,
    pub properties: usize,
    pub events: usize,
    pub methods: usize,
}

#[derive(Serialize, Clone, Debug)]
pub struct DangerRefOut {
    pub kind: String,
    pub name: String,
    pub address: String,
    pub declared_in: String,
    #[serde(flatten)]
    pub danger: DangerOut,
}

/// An entity card (`svelters docs gtk button`).
#[derive(Serialize, Clone, Debug)]
pub struct Card {
    pub entity: EntityOut,
    /// `single_child` | `named_slots` | `none` | `unknown` …; absent when the IR has no model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_slot: Option<String>,
    /// Own members; inherited ones too with `--all` (see `declared_in`).
    pub properties: Vec<MemberOut>,
    pub events: Vec<MemberOut>,
    pub slots: Vec<MemberOut>,
    pub methods: Vec<MemberOut>,
    pub constructors: Vec<MemberOut>,
    pub values: Vec<MemberOut>,
    /// Own and inherited dangerous members (always complete, also without `--all`).
    pub dangerous: Vec<DangerRefOut>,
    /// Counts of inherited members per ancestor (complete whether or not `--all` is given).
    pub inherited: Vec<InheritedOut>,
    /// Whether the member arrays include inherited members (`--all`).
    pub all: bool,
    pub examples: Vec<Example>,
}

pub fn card(pack: &Pack, i: usize, brief: bool, all: bool) -> Card {
    let e = pack.entity(i);
    let list = |kind: MemberKind| -> Vec<MemberOut> {
        resolve::visible_members(pack, i, kind)
            .iter()
            .filter(|h| all || h.owner == i || matches!(kind, MemberKind::Slot | MemberKind::Attr))
            .map(|h| member_out(pack, i, h, brief))
            .collect()
    };
    let mut inherited: Vec<InheritedOut> = Vec::new();
    let mut dangerous = Vec::new();
    for kind in [MemberKind::Property, MemberKind::Event, MemberKind::Method] {
        for h in resolve::visible_members(pack, i, kind) {
            if let Some(m) = Some(member_out(pack, i, &h, true))
                && let Some(d) = &m.base().dangerous
            {
                dangerous.push(DangerRefOut {
                    kind: kind.word().into(),
                    name: m.base().name.clone(),
                    address: m.base().address.clone(),
                    declared_in: m.base().declared_in.clone(),
                    danger: d.clone(),
                });
            }
            if h.owner == i {
                continue;
            }
            let from = &pack.header.entities[h.owner];
            let row = match inherited.iter_mut().find(|r| r.from == from.id) {
                Some(r) => r,
                None => {
                    inherited.push(InheritedOut {
                        from: from.id.clone(),
                        name: from.name.clone(),
                        properties: 0,
                        events: 0,
                        methods: 0,
                    });
                    inherited.last_mut().unwrap()
                }
            };
            match kind {
                MemberKind::Property => row.properties += 1,
                MemberKind::Event => row.events += 1,
                _ => row.methods += 1,
            }
        }
    }
    let ch = resolve::slot_owner(pack, &pack.chain(i))
        .map(|o| pack.entity(o).children.clone().unwrap_or_default());
    let own_ch = e
        .children
        .as_ref()
        .filter(|c| !c.slots.is_empty() || c.policy != "unknown");
    let model = own_ch.cloned().or(ch);
    Card {
        entity: entity_out(pack, i, brief),
        children_policy: model.as_ref().map(|c| c.policy.clone()),
        default_slot: model.as_ref().and_then(|c| c.default_slot.clone()),
        properties: list(MemberKind::Property),
        events: list(MemberKind::Event),
        slots: list(MemberKind::Slot),
        methods: list(MemberKind::Method),
        constructors: list(MemberKind::Constructor),
        values: list(MemberKind::Value),
        dangerous,
        inherited,
        all,
        examples: if brief {
            Vec::new()
        } else {
            e.examples.clone()
        },
    }
}

/// A short entity reference (member results, index rows).
#[derive(Serialize, Clone, Debug)]
pub struct EntityRef {
    pub id: String,
    pub address: String,
    pub native: String,
    pub tags: Vec<String>,
    pub site: String,
}

pub fn entity_ref(pack: &Pack, i: usize) -> EntityRef {
    let d = &pack.header.entities[i];
    EntityRef {
        id: d.id.clone(),
        address: resolve::entity_address(pack, i),
        native: d.native_name.clone(),
        tags: d.tags.clone(),
        site: d.route.clone(),
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct IndexRow {
    pub address: String,
    pub id: String,
    pub native: String,
    pub tags: Vec<String>,
    pub kind: String,
    pub tier: String,
    pub tested: bool,
    pub brief: String,
    pub site: String,
}

pub fn index_row(pack: &Pack, i: usize) -> IndexRow {
    let d = &pack.header.entities[i];
    IndexRow {
        address: resolve::entity_address(pack, i),
        id: d.id.clone(),
        native: d.native_name.clone(),
        tags: d.tags.clone(),
        kind: d.kind.clone(),
        tier: d.tier.clone(),
        tested: d.tested,
        brief: d.brief.clone(),
        site: d.route.clone(),
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct LicenseOut {
    pub spdx: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// `full` | `brief_only` | `link_only`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redistribution: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ToolkitOut {
    pub id: String,
    pub name: String,
    pub version: String,
    pub data_version: String,
    /// Where the pack came from.
    pub source: String,
    pub elements: usize,
    pub entities: usize,
    pub doc_license: LicenseOut,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_policy: Option<String>,
    pub notices: Vec<String>,
    /// Labels of `extensions.<toolkit>` fields.
    pub extension_labels: BTreeMap<String, String>,
}

pub fn toolkit_out(pack: &Pack) -> ToolkitOut {
    let h = &pack.header;
    let mut labels = BTreeMap::new();
    for schema in h.extension_schemas.values() {
        for f in schema
            .get("fields")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let (Some(k), Some(l)) = (
                f.get("key").and_then(Value::as_str),
                f.get("label").and_then(Value::as_str),
            ) {
                labels.insert(k.to_string(), l.to_string());
            }
        }
    }
    ToolkitOut {
        id: h.toolkit.id.clone(),
        name: h.toolkit.name.clone(),
        version: h.toolkit.version.clone(),
        data_version: h.data_version.clone(),
        source: pack.source.describe(),
        elements: h.entities.iter().filter(|d| !d.tags.is_empty()).count(),
        entities: h.entities.len(),
        doc_license: LicenseOut {
            spdx: h.toolkit.doc_license.spdx.clone(),
            name: h.toolkit.doc_license.name.clone(),
            url: h.toolkit.doc_license.url.clone(),
            redistribution: h.toolkit.doc_license.redistribution.clone(),
        },
        text_policy: h.toolkit.text_policy.clone(),
        notices: h.toolkit.notices.clone(),
        extension_labels: labels,
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Candidate {
    pub address: String,
    pub toolkit: String,
    /// `element` | `type` | a member kind.
    pub kind: String,
    /// Entity id that declares an inherited member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_in: Option<String>,
    pub brief: String,
    pub site: String,
}

pub fn entity_candidate(pack: &Pack, i: usize) -> Candidate {
    let d = &pack.header.entities[i];
    Candidate {
        address: resolve::entity_address(pack, i),
        toolkit: pack.toolkit().into(),
        kind: if d.tags.is_empty() {
            "type".into()
        } else {
            "element".into()
        },
        declared_in: None,
        brief: d.brief.clone(),
        site: d.route.clone(),
    }
}

pub fn member_candidate(pack: &Pack, entity: usize, h: &Hit) -> Candidate {
    let m = member_out(pack, entity, h, false);
    let b = m.base();
    Candidate {
        address: b.address.clone(),
        toolkit: pack.toolkit().into(),
        kind: b.kind.clone(),
        // only for inherited members
        declared_in: (h.owner != entity).then(|| b.declared_in.clone()),
        brief: b
            .doc
            .as_deref()
            .map(text::first_paragraph)
            .map(|s| text::plain(&s))
            .unwrap_or_default(),
        site: b.site.clone(),
    }
}

/// The JSON body of one command result, tagged by `kind`.
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Body {
    /// `svelters docs`: the available toolkits.
    Toolkits {
        #[serde(skip_serializing_if = "Option::is_none")]
        project: Option<ProjectOut>,
        toolkits: Vec<ToolkitOut>,
    },
    /// `svelters docs <tk>`: elements (and with `--all` every type).
    Index {
        toolkit: Box<ToolkitOut>,
        elements: Vec<IndexRow>,
        types: Vec<IndexRow>,
        types_total: usize,
    },
    /// `svelters docs <tk> <entity>`.
    Entity {
        toolkit: String,
        data_version: String,
        #[serde(flatten)]
        card: Box<Card>,
        notices: Vec<String>,
    },
    /// `svelters docs <tk> <entity>.<member>`.
    Member {
        toolkit: String,
        data_version: String,
        entity: EntityRef,
        member: Box<MemberOut>,
        notices: Vec<String>,
    },
    /// `-k <query>`.
    Search {
        query: String,
        toolkits: Vec<String>,
        results: Vec<Candidate>,
        truncated: bool,
    },
    /// Several matches; nothing was chosen (exit 2).
    Ambiguous {
        query: String,
        candidates: Vec<Candidate>,
    },
    /// No match (exit 1); `suggestions` are near names.
    NotFound {
        query: String,
        toolkits: Vec<String>,
        suggestions: Vec<Candidate>,
    },
}

#[derive(Serialize, Clone, Debug)]
pub struct ProjectOut {
    pub toolkit: String,
    pub config: String,
}

/// The envelope: `{"schema": "svelters-docs/1", "kind": …, …, "notes": [...]}`.
#[derive(Serialize, Clone, Debug)]
pub struct Envelope {
    pub schema: &'static str,
    #[serde(flatten)]
    pub body: Body,
    /// Informational lines (toolkit chosen from svelters.toml, unique-toolkit notes, warnings).
    pub notes: Vec<String>,
}
