//! Terminal markdown for `svelters docs` (the default format) and `--brief`. Reads only the output
//! model in `out.rs`, the same values `--json` prints.

use std::fmt::Write;

use crate::out::{
    Body, Candidate, Card, DangerOut, EntityOut, IndexRow, MemberOut, ToolkitOut, TypeOut,
};
use crate::text;

pub struct Style {
    /// ANSI bold headings (stdout is a terminal and NO_COLOR is unset).
    pub color: bool,
    pub brief: bool,
}

fn h(out: &mut String, st: &Style, level: usize, s: &str) {
    let hashes = "#".repeat(level);
    if st.color {
        let _ = writeln!(out, "\x1b[1m{hashes} {s}\x1b[0m");
    } else {
        let _ = writeln!(out, "{hashes} {s}");
    }
}

fn severity(s: &str) -> &str {
    match s {
        "corrupts-tree" => "corrupts the widget tree",
        "owned-by-backend" => "owned by the backend",
        "crashes" => "can crash",
        _ => "backend-owned state",
    }
}

fn ty(t: &TypeOut) -> String {
    let mut s = format!("`{}`", t.display.replace('`', "'"));
    if t.nullable {
        s.push_str(" or `null`");
    }
    s
}

fn one_line(s: &str) -> String {
    text::first_paragraph(s)
}

fn entity_title(e: &EntityOut) -> String {
    match e.tags.first() {
        Some(t) => format!("<{t}> · {}", e.native),
        None => e.native.clone(),
    }
}

fn status(e: &EntityOut) -> String {
    let mut s = if e.tier == "annotated" {
        "annotated".to_string()
    } else {
        "ingested (not annotated)".to_string()
    };
    if e.tested {
        s.push_str(", tested");
    }
    if let Some((v, t)) = e.verified_facts
        && t > 0
    {
        let _ = write!(s, " ({v}/{t} facts verified)");
    }
    s
}

fn short_id(id: &str) -> &str {
    id.split_once(':').map_or(id, |(_, r)| r)
}

fn danger_block(out: &mut String, d: &DangerOut, what: &str) {
    let how = if d.reachable {
        format!("{what} is reachable from app code, but the backend owns what it changes.")
    } else {
        format!("{what} is not possible from app code today; the backend owns this state.")
    };
    let _ = writeln!(
        out,
        "> ⚠ **Dangerous: {}.** {how} {}",
        severity(&d.severity),
        one_line(&d.reason)
    );
    if let Some(u) = &d.use_instead {
        let _ = writeln!(out, "> **Use instead:** {}", one_line(u));
    }
    let _ = writeln!(
        out,
        "> Declared in `{}`. Nothing blocks it at run time yet.",
        d.source
    );
    out.push('\n');
}

fn member_line(m: &MemberOut, card_id: &str, brief: bool) -> String {
    let b = m.base();
    let mut s = String::new();
    match m {
        MemberOut::Property {
            ty: t,
            access,
            default,
            bindable,
            ..
        } => {
            let _ = write!(s, "- `{}`: {}, {access}", b.name, ty(t));
            if let Some(d) = default {
                let _ = write!(s, ", default `{d}`");
            }
            if *bindable {
                s.push_str(", bindable");
            }
        }
        MemberOut::Event {
            attr, params, sync, ..
        } => {
            let args: Vec<String> = params
                .iter()
                .map(|p| format!("{}: {}", p.name, p.ty.display))
                .collect();
            let _ = write!(s, "- `{}`", b.name);
            if let Some(a) = attr {
                let _ = write!(s, " (`{a}`)");
            }
            let _ = write!(s, " ({})", args.join(", "));
            if *sync {
                s.push_str(", sync");
            }
        }
        MemberOut::Method {
            signatures,
            is_static,
            ..
        } => {
            let _ = write!(s, "- `{}`", signatures[0].replace('`', "'"));
            if signatures.len() > 1 {
                let _ = write!(s, " (+{} overloads)", signatures.len() - 1);
            }
            if *is_static {
                s.push_str(", static");
            }
        }
        MemberOut::Value { value, nick, .. } => {
            let _ = write!(s, "- `{}` = `{}`", b.name, value);
            if let Some(n) = nick
                && n != &b.name
            {
                let _ = write!(s, " (`{n}`)");
            }
        }
        MemberOut::Slot {
            default,
            cardinality,
            accepts,
            attrs,
            ..
        } => {
            let _ = write!(
                s,
                "- `{}`{}: {cardinality} {}",
                b.name,
                if *default { " (default)" } else { "" },
                accepts.join(" | ")
            );
            if !attrs.is_empty() {
                let _ = write!(
                    s,
                    "; child attrs {}",
                    attrs
                        .iter()
                        .map(|a| format!("`parent:{a}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
        MemberOut::Attr {
            attribute, slot, ..
        } => {
            let _ = write!(s, "- `{attribute}` (slot `{slot}`)");
        }
    }
    if let Some(since) = &b.since {
        let _ = write!(s, ", since {since}");
    }
    if b.deprecated.is_some() {
        s.push_str(", deprecated");
    }
    if b.dangerous.is_some() {
        s.push_str(" ⚠ dangerous");
    }
    if b.declared_in != card_id {
        let _ = write!(s, " (from {})", short_id(&b.declared_in));
    }
    if !brief && let Some(d) = &b.doc {
        let _ = write!(s, " — {}", one_line(d));
    }
    s
}

fn section(out: &mut String, st: &Style, title: &str, list: &[MemberOut], card_id: &str) {
    if list.is_empty() {
        return;
    }
    h(out, st, 2, title);
    for m in list {
        out.push_str(&member_line(m, card_id, st.brief));
        out.push('\n');
    }
    out.push('\n');
}

fn brief_names(list: &[MemberOut], show_type: bool) -> String {
    list.iter()
        .map(|m| match m {
            MemberOut::Property { ty: t, access, .. } if show_type => {
                format!("{} {} {access}", m.base().name, t.display)
            }
            _ => m.base().name.clone(),
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

pub fn card(st: &Style, c: &Card, toolkit_name: &str, notices: &[String]) -> String {
    let mut o = String::new();
    let e = &c.entity;
    let id = e.id.as_str();
    if st.brief {
        let _ = writeln!(
            o,
            "{} ({}) · {} {} · {}",
            entity_title(e),
            e.id,
            toolkit_name,
            e.kind,
            status(e)
        );
        if !e.bases.is_empty() {
            let _ = writeln!(
                o,
                "inherits: {}",
                e.bases
                    .iter()
                    .map(|b| short_id(b))
                    .collect::<Vec<_>>()
                    .join(" → ")
            );
        }
        if let Some(s) = &e.summary {
            let _ = writeln!(o, "{}", text::plain(s));
        }
        let rows: [(&str, &[MemberOut], bool); 6] = [
            ("props", &c.properties, true),
            ("events", &c.events, false),
            ("slots", &c.slots, false),
            ("methods", &c.methods, false),
            ("ctors", &c.constructors, false),
            ("values", &c.values, false),
        ];
        for (label, list, typed) in rows {
            if !list.is_empty() {
                let _ = writeln!(o, "{label}: {}", brief_names(list, typed));
            }
        }
        if !c.dangerous.is_empty() {
            let _ = writeln!(
                o,
                "dangerous: {}",
                c.dangerous
                    .iter()
                    .map(|d| format!("{}:{}", d.kind, d.name))
                    .collect::<Vec<_>>()
                    .join(" · ")
            );
        }
        if !c.all && !c.inherited.is_empty() {
            let _ = writeln!(
                o,
                "inherited props/events/methods: {} (--all)",
                c.inherited
                    .iter()
                    .map(|i| format!("{} {}/{}/{}", i.name, i.properties, i.events, i.methods))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        return o;
    }

    h(&mut o, st, 1, &entity_title(e));
    let mut line = format!(
        "`{}` · {toolkit_name} {}{}",
        e.id,
        e.kind,
        if e.is_abstract { " (abstract)" } else { "" }
    );
    let _ = write!(line, " · {}", status(e));
    if let Some(l) = &e.lifetime {
        let _ = write!(line, " · lifetime {l}");
    }
    if let Some(s) = &e.since {
        let _ = write!(line, " · since {s}");
    }
    let _ = writeln!(o, "{line}");
    if !e.bases.is_empty() {
        let _ = writeln!(
            o,
            "Inherits: {}",
            e.bases
                .iter()
                .map(|b| short_id(b))
                .collect::<Vec<_>>()
                .join(" → ")
        );
    }
    if !e.interfaces.is_empty() {
        let _ = writeln!(
            o,
            "Interfaces: {}",
            e.interfaces
                .iter()
                .map(|b| short_id(b))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let _ = write!(o, "Site: {}", e.site);
    if let Some(u) = &e.upstream {
        let _ = write!(o, " · Upstream: {u}");
    }
    o.push_str("\n\n");
    // GTK descriptions start with the summary sentence; print it once
    if let Some(s) = &e.summary
        && !e
            .description
            .as_ref()
            .is_some_and(|d| d.starts_with(s.as_str()))
    {
        let _ = writeln!(o, "{s}\n");
    }
    if let Some(d) = &e.description {
        h(&mut o, st, 2, "Description");
        let _ = writeln!(o, "{d}\n");
    }
    section(&mut o, st, "Properties", &c.properties, id);
    section(&mut o, st, "Events", &c.events, id);
    if !c.slots.is_empty() || c.children_policy.is_some() {
        h(&mut o, st, 2, "Children");
        if let Some(p) = &c.children_policy {
            let _ = write!(o, "Policy `{p}`");
            if let Some(d) = &c.default_slot {
                let _ = write!(o, ", default slot `{d}`");
            }
            o.push('\n');
        }
        for m in &c.slots {
            o.push_str(&member_line(m, id, true));
            o.push('\n');
        }
        o.push('\n');
    }
    if !c.dangerous.is_empty() {
        h(&mut o, st, 2, "Dangerous members");
        for d in &c.dangerous {
            let from = if d.declared_in != id {
                format!(" (from {})", short_id(&d.declared_in))
            } else {
                String::new()
            };
            let reach = if d.danger.reachable {
                ""
            } else {
                ", not reachable today"
            };
            let _ = write!(
                o,
                "- {} `{}`{from}: {}{reach}. {}",
                d.kind,
                d.name,
                severity(&d.danger.severity),
                one_line(&d.danger.reason)
            );
            if let Some(u) = &d.danger.use_instead {
                let _ = write!(o, " Use instead: {}", one_line(u));
            }
            o.push('\n');
        }
        o.push('\n');
    }
    section(&mut o, st, "Constructors", &c.constructors, id);
    section(&mut o, st, "Methods", &c.methods, id);
    section(&mut o, st, "Values", &c.values, id);
    if !c.all && !c.inherited.is_empty() {
        let parts: Vec<String> = c
            .inherited
            .iter()
            .map(|i| {
                let mut v = Vec::new();
                for (n, w) in [
                    (i.properties, "propert"),
                    (i.events, "event"),
                    (i.methods, "method"),
                ] {
                    if n > 0 {
                        let plural = match (n, w) {
                            (1, "propert") => "y",
                            (_, "propert") => "ies",
                            (1, _) => "",
                            _ => "s",
                        };
                        v.push(format!("{n} {w}{plural}"));
                    }
                }
                format!("{} ({})", i.name, v.join(", "))
            })
            .collect();
        let _ = writeln!(
            o,
            "Inherited: {}. List them with `--all`.\n",
            parts.join("; ")
        );
    }
    if !e.extensions.is_empty() {
        for (tk, v) in &e.extensions {
            if let Some(obj) = v.as_object() {
                let short: Vec<String> = obj
                    .iter()
                    .filter_map(|(k, v)| {
                        v.as_str()
                            .filter(|s| !s.is_empty() && !s.contains('\n') && s.len() <= 80)
                            .map(|s| format!("{k} `{s}`"))
                    })
                    .collect();
                if !short.is_empty() {
                    let _ = writeln!(o, "{tk}: {}\n", short.join(" · "));
                }
            }
        }
    }
    for ex in &c.examples {
        h(&mut o, st, 2, &format!("Example: {}", ex.title));
        let _ = writeln!(
            o,
            "```{}\n{}\n```{}\n",
            ex.lang,
            ex.code.trim_end(),
            if ex.generated {
                "\n(generated from metadata, illustrative)"
            } else {
                ""
            }
        );
    }
    notices_block(&mut o, notices, c_has_text(c));
    o
}

fn c_has_text(c: &Card) -> bool {
    c.entity.summary.is_some()
        || c.entity.description.is_some()
        || [&c.properties, &c.events, &c.methods]
            .iter()
            .any(|l| l.iter().any(|m| m.base().doc.is_some()))
}

fn notices_block(o: &mut String, notices: &[String], has_text: bool) {
    if has_text && !notices.is_empty() {
        let _ = writeln!(o, "---\n{}", notices.join(" "));
    }
}

pub fn member(
    st: &Style,
    entity: &crate::out::EntityRef,
    m: &MemberOut,
    notices: &[String],
) -> String {
    let b = m.base();
    let mut o = String::new();
    let owner = entity
        .tags
        .first()
        .map(|t| format!("<{t}>"))
        .unwrap_or_else(|| entity.native.clone());
    if st.brief {
        let line = member_line(m, &entity.id, true);
        let _ = writeln!(
            o,
            "{} {} on {owner}: {}",
            b.kind,
            b.name,
            line.trim_start_matches("- ")
        );
        return o;
    }
    h(&mut o, st, 1, &format!("{owner}.{} · {}", b.name, b.kind));
    let decl = if b.declared_in == entity.id {
        "declared here".to_string()
    } else {
        format!("declared in {}", short_id(&b.declared_in))
    };
    let _ = write!(o, "`{}` · {decl}", b.address);
    if !b.site.is_empty() {
        let _ = write!(o, " · Site: {}", b.site);
    }
    o.push('\n');
    let mut facts: Vec<String> = Vec::new();
    match m {
        MemberOut::Property {
            native,
            ty: t,
            access,
            default,
            range,
            notify,
            bindable,
            accessors,
            ..
        } => {
            facts.push(format!("Type {}", ty(t)));
            facts.push(access.clone());
            if let Some(d) = default {
                facts.push(format!("default `{d}`"));
            }
            if let Some(r) = range {
                facts.push(format!("range `{r}`"));
            }
            if let Some(n) = notify {
                facts.push(format!("notify `{n}`"));
            }
            if *bindable {
                facts.push(format!("bindable (`bind:{}`)", b.name));
            }
            if native != &b.name {
                facts.push(format!("native name `{native}`"));
            }
            if !accessors.is_empty() {
                facts.push(format!(
                    "accessors {}",
                    accessors
                        .iter()
                        .map(|(k, v)| format!("{k} `{v}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        MemberOut::Event {
            native,
            attr,
            aliases,
            params,
            returns,
            sync,
            result,
            phase,
            flags,
            ..
        } => {
            let mut hs: Vec<String> = attr.iter().cloned().collect();
            hs.extend(aliases.iter().cloned());
            if !hs.is_empty() {
                facts.push(format!(
                    "Handler {}",
                    hs.iter()
                        .map(|x| format!("`{x}`"))
                        .collect::<Vec<_>>()
                        .join(" / ")
                ));
            }
            let args: Vec<String> = params
                .iter()
                .map(|p| format!("{}: {}", p.name, ty(&p.ty)))
                .collect();
            facts.push(format!(
                "arguments {}",
                if args.is_empty() {
                    "none".into()
                } else {
                    args.join(", ")
                }
            ));
            if let Some(r) = returns {
                facts.push(format!("returns {}", ty(r)));
            }
            facts.push(if *sync {
                "sync required".into()
            } else {
                "async".into()
            });
            if let Some(r) = result.as_ref().filter(|r| r.as_str() != "none") {
                facts.push(format!("result `{r}`"));
            }
            if let Some(p) = phase {
                facts.push(format!("phase `{p}`"));
            }
            if !flags.is_empty() {
                facts.push(format!("flags {}", flags.join(", ")));
            }
            if native != &b.name {
                facts.push(format!("native signal `{native}`"));
            }
        }
        MemberOut::Method {
            signatures,
            is_static,
            is_virtual,
            unsupported,
            ..
        } => {
            o.push('\n');
            for s in signatures {
                let _ = writeln!(o, "    {s}");
            }
            if *is_static {
                facts.push("static".into());
            }
            if *is_virtual {
                facts.push("virtual".into());
            }
            if let Some(u) = unsupported {
                facts.push(format!("unsupported: {u}"));
            }
        }
        MemberOut::Value { value, nick, .. } => {
            facts.push(format!("value `{value}`"));
            if let Some(n) = nick {
                facts.push(format!("nick `{n}`"));
            }
        }
        MemberOut::Slot {
            default,
            cardinality,
            accepts,
            attrs,
            order,
            ops,
            ownership,
            on_remove,
            ..
        } => {
            facts.push(format!(
                "{cardinality} child{}",
                if *default { ", default slot" } else { "" }
            ));
            facts.push(format!("accepts {}", accepts.join(" | ")));
            if let Some(o) = order {
                facts.push(format!("order `{o}`"));
            }
            if !attrs.is_empty() {
                facts.push(format!(
                    "child attrs {}",
                    attrs
                        .iter()
                        .map(|a| format!("`parent:{a}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            for (k, v) in ops {
                facts.push(format!("{k} `{v}`"));
            }
            for (k, v) in ownership {
                facts.push(format!("on {k}: {v}"));
            }
            if let Some(r) = on_remove {
                facts.push(format!("on remove `{r}`"));
            }
        }
        MemberOut::Attr {
            attribute, slot, ..
        } => {
            facts.push(format!("written on the child as `{attribute}={{…}}`"));
            facts.push(format!(
                "given by slot `{slot}`; see `{}.slot:{slot}`",
                entity.address
            ));
        }
    }
    if let Some(s) = &b.since {
        facts.push(format!("since {s}"));
    }
    if let Some(d) = &b.deprecated {
        facts.push(if d.as_bool() == Some(true) {
            "deprecated".into()
        } else {
            format!("deprecated since {}", d.as_str().unwrap_or("?"))
        });
    }
    let mut prov = Vec::new();
    if b.annotated {
        prov.push("annotated");
    }
    if b.verified {
        prov.push("verified");
    }
    if !prov.is_empty() {
        facts.push(prov.join(", "));
    }
    let _ = writeln!(o, "{}\n", facts.join(" · "));
    if let Some(d) = &b.dangerous {
        let what = match m {
            MemberOut::Property { .. } => "Writing this property".to_string(),
            MemberOut::Event { .. } => "Using this event".to_string(),
            _ => format!("Calling `{}`", b.name),
        };
        danger_block(&mut o, d, &what);
    }
    if let Some(d) = &b.doc {
        let _ = writeln!(o, "{d}\n");
    }
    notices_block(&mut o, notices, b.doc.is_some());
    o
}

fn index_line(r: &IndexRow) -> String {
    let name = r
        .tags
        .first()
        .map(|t| format!("`<{t}>` {}", r.native))
        .unwrap_or_else(|| {
            format!(
                "`{}`",
                r.address
                    .split_once(' ')
                    .map_or(r.address.as_str(), |x| x.1)
            )
        });
    let mut s = format!("- {name}");
    if r.tier != "annotated" {
        s.push_str(" (ingested)");
    }
    if !r.brief.is_empty() {
        let _ = write!(s, " — {}", r.brief);
    }
    s
}

pub fn render(st: &Style, body: &Body) -> String {
    let mut o = String::new();
    match body {
        Body::Toolkits { project, toolkits } => {
            h(&mut o, st, 1, "svelters docs");
            match project {
                Some(p) => {
                    let _ = writeln!(o, "Project toolkit: {} ({})", p.toolkit, p.config);
                }
                None => o.push_str("No svelters.toml here: name a toolkit, or the CLI picks the one that has the name.\n"),
            }
            o.push('\n');
            for t in toolkits {
                toolkit_line(&mut o, t);
            }
            if toolkits.is_empty() {
                o.push_str("No docs packs found. In the repo run `pnpm docs:ir`; elsewhere pass --pack <file>.\n");
            }
            o.push_str("\nUsage: svelters docs [<toolkit>] <element>[.<member>] [--brief|--json|--all] · -k <query> · --help\n");
        }
        Body::Index {
            toolkit,
            elements,
            types,
            types_total,
        } => {
            h(
                &mut o,
                st,
                1,
                &format!("{} elements ({})", toolkit.name, toolkit.id),
            );
            toolkit_line(&mut o, toolkit);
            o.push('\n');
            for r in elements {
                let _ = writeln!(o, "{}", index_line(r));
            }
            if types.is_empty() {
                let _ = writeln!(
                    o,
                    "\n{types_total} more types (enums, interfaces, non-element classes): `svelters docs {} --all`.",
                    toolkit.id
                );
            } else {
                o.push('\n');
                h(&mut o, st, 2, "Types");
                for r in types {
                    let _ = writeln!(o, "{}", index_line(r));
                }
            }
        }
        Body::Entity {
            card: c,
            notices,
            toolkit,
            ..
        } => {
            o.push_str(&card(st, c, toolkit, notices));
        }
        Body::Member {
            entity,
            member: m,
            notices,
            ..
        } => {
            o.push_str(&member(st, entity, m, notices));
        }
        Body::Search {
            query,
            results,
            truncated,
            ..
        } => {
            if results.is_empty() {
                let _ = writeln!(o, "Nothing matches `{query}`.");
            }
            for c in results {
                o.push_str(&cand_line(c));
            }
            if *truncated {
                o.push_str("(more results: --limit <n>)\n");
            }
        }
        Body::Ambiguous { query, candidates } => {
            let _ = writeln!(o, "`{query}` is ambiguous; use one of:");
            for c in candidates {
                o.push_str(&cand_line(c));
            }
        }
        Body::NotFound {
            query,
            toolkits,
            suggestions,
        } => {
            let _ = writeln!(o, "No `{query}` in {}.", toolkits.join(", "));
            if !suggestions.is_empty() {
                o.push_str("Did you mean:\n");
                for c in suggestions {
                    o.push_str(&cand_line(c));
                }
            }
        }
    }
    o
}

fn toolkit_line(o: &mut String, t: &ToolkitOut) {
    let _ = writeln!(
        o,
        "- {}: {} {}, {} elements, {} types; data {} ({}); docs text {}{}",
        t.id,
        t.name,
        t.version,
        t.elements,
        t.entities,
        t.data_version,
        t.source,
        t.doc_license.spdx,
        t.text_policy
            .as_deref()
            .map(|p| format!(", policy {p}"))
            .unwrap_or_default()
    );
}

fn cand_line(c: &Candidate) -> String {
    let mut s = format!("  {}  ({}", c.address, c.kind);
    if let Some(d) = &c.declared_in {
        let _ = write!(s, ", from {}", short_id(d));
    }
    s.push(')');
    if !c.brief.is_empty() {
        let b: String = c.brief.chars().take(100).collect();
        let _ = write!(
            s,
            "  {b}{}",
            if c.brief.chars().count() > 100 {
                "…"
            } else {
                ""
            }
        );
    }
    s.push('\n');
    s
}
