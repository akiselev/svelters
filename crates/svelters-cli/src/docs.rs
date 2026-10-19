//! `svelters docs` and `svelters docs-pack`.

use std::path::PathBuf;

use crate::locate::{self, Search};
use crate::md::{self, Style};
use crate::out::{self, Body, Candidate, Envelope, ProjectOut};
use crate::pack::Pack;
use crate::resolve::{self, Hit, Member, MemberKind, MemberQuery};

pub const EXIT_OK: i32 = 0;
pub const EXIT_NOT_FOUND: i32 = 1;
pub const EXIT_AMBIGUOUS: i32 = 2;
pub const EXIT_USAGE: i32 = 3;
pub const EXIT_NO_DATA: i32 = 4;

pub const DOCS_HELP: &str = "\
svelters docs: offline API docs for svelters elements (the same data as the docs site)

USAGE
  svelters docs                              toolkits, project toolkit, data versions
  svelters docs <toolkit>                    element index (--all: every type too)
  svelters docs [<toolkit>] <element>        the element card
  svelters docs [<toolkit>] <element>.<member>
  svelters docs -k <query>                   search names and summaries

ADDRESSES (case-insensitive; `go doc` style)
  element   a tag (button, main-window), a native class (GtkButton, Gtk.Button,
            QMainWindow), or an entity id (gtk:Gtk.Button); enums and other types too
  member    a property, event (clicked / onclicked), method, constructor, enum value,
            slot or child attribute, searched on the element and then its bases;
            force a kind with prop: event: method: ctor: value: slot: attr:
            (main-window.prop:title, grid.attr:row). `::` also separates
            (QMainWindow::setCentralWidget).

TOOLKIT
  The first word when it names a toolkit, or -t <toolkit>; else `[app] toolkit` in the
  nearest svelters.toml; else the toolkit that has the name (several: you are asked).

OUTPUT
  (default)      markdown
  --brief        names and types only, no doc text (about 20 lines per element)
  --json         stable JSON, schema svelters-docs/1 (plans/milestones/CL0-cli-docs.md)
  --all          include inherited members (cards) / all types (index)
  --section <s>  one card section: properties events slots methods constructors values
                 dangerous examples description
  --limit <n>    search results (default 20)

DATA
  --pack <file|dir>   use these docs packs only (also SVELTERS_DOCS_PACK, `:`-separated)
  --ir <file|dir>     build packs in memory from docs IR JSON
  In the svelters repo the pack is rebuilt from docs/site/.generated/ir when stale.

EXIT STATUS
  0 found · 1 not found (suggestions printed) · 2 ambiguous (candidates printed)
  3 usage error · 4 no docs data
";

pub const PACK_HELP: &str = "\
svelters docs-pack: build docs packs from the docs IR (the site's input)

USAGE
  svelters docs-pack [--ir <dir|file>] [--out <dir>] [--check]

  Defaults (inside the repo): --ir docs/site/.generated/ir --out docs/site/.generated/pack.
  --check  rebuild in memory and fail if the packs in --out differ (stale or missing)
";

/// Process context, filled by `main` from the real environment and by tests directly.
#[derive(Clone, Debug, Default)]
pub struct Ctx {
    pub cwd: PathBuf,
    /// `SVELTERS_DOCS_PACK` split on `:`.
    pub env_packs: Vec<PathBuf>,
    /// Only the explicit pack sources (tests).
    pub explicit_only: bool,
    /// stdout is a terminal and NO_COLOR is unset.
    pub color: bool,
}

pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Default)]
struct Opts {
    brief: bool,
    json: bool,
    all: bool,
    search: Option<String>,
    toolkit: Option<String>,
    section: Option<String>,
    limit: usize,
    packs: Vec<PathBuf>,
    irs: Vec<PathBuf>,
    positional: Vec<String>,
}

fn usage(msg: String) -> Output {
    Output {
        code: EXIT_USAGE,
        stdout: String::new(),
        stderr: format!("svelters docs: {msg}\nRun `svelters docs --help` for usage.\n"),
    }
}

fn parse(args: &[String]) -> Result<Option<Opts>, String> {
    let mut o = Opts {
        limit: 20,
        ..Default::default()
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let (flag, inline) = match a.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v.to_string())),
            _ => (a.as_str(), None),
        };
        let mut val = |name: &str| -> Result<String, String> {
            inline
                .clone()
                .or_else(|| it.next().cloned())
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match flag {
            "-h" | "--help" => return Ok(None),
            "--brief" => o.brief = true,
            "--json" => o.json = true,
            "--all" | "-a" => o.all = true,
            "-k" | "--search" => o.search = Some(val(flag)?),
            "-t" | "--toolkit" => o.toolkit = Some(val(flag)?),
            "--section" => o.section = Some(val(flag)?),
            "--limit" => {
                o.limit = val(flag)?
                    .parse()
                    .map_err(|_| "--limit needs a number".to_string())?
            }
            "--pack" => o.packs.push(val(flag)?.into()),
            "--ir" => o.irs.push(val(flag)?.into()),
            "--" => o.positional.extend(it.by_ref().cloned()),
            f if f.starts_with('-') && f.len() > 1 => return Err(format!("unknown option {f}")),
            _ => o.positional.push(a.clone()),
        }
    }
    if o.positional.len() > 2 {
        return Err(format!("too many arguments: {}", o.positional.join(" ")));
    }
    Ok(Some(o))
}

/// What one toolkit says about an address.
enum Res<'a> {
    Entity(usize),
    Member(usize, Hit<'a>),
    Ambiguous(Vec<Candidate>),
    EntityMiss,
    MemberMiss(usize, MemberQuery),
}

fn resolve_in<'a>(pack: &'a Pack, addr: &str) -> Res<'a> {
    for (ent, mq) in resolve::splits(addr) {
        let found = resolve::find_entities(pack, &ent);
        if found.is_empty() {
            continue;
        }
        if found.len() > 1 {
            return Res::Ambiguous(
                found
                    .iter()
                    .map(|&i| out::entity_candidate(pack, i))
                    .collect(),
            );
        }
        let i = found[0];
        let Some(q) = mq else { return Res::Entity(i) };
        let mut hits = resolve::find_members(pack, i, &q);
        return match hits.len() {
            0 => Res::MemberMiss(i, q),
            1 => Res::Member(i, hits.remove(0)),
            _ => Res::Ambiguous(
                hits.iter()
                    .map(|h| out::member_candidate(pack, i, h))
                    .collect(),
            ),
        };
    }
    Res::EntityMiss
}

fn search(packs: &[&Pack], query: &str, limit: usize) -> (Vec<Candidate>, bool) {
    let q = query.to_lowercase();
    let qn = resolve::norm(query);
    let score = |name: &str| -> Option<usize> {
        let l = name.to_lowercase();
        let n = resolve::norm(name);
        if l == q || (!qn.is_empty() && n == qn) {
            Some(0)
        } else if l.starts_with(&q) || (!qn.is_empty() && n.starts_with(&qn)) {
            Some(1)
        } else if l.contains(&q) || (!qn.is_empty() && n.contains(&qn)) {
            Some(2)
        } else {
            None
        }
    };
    // (score, rank, address, candidate)
    let mut hits: Vec<(usize, usize, String, Candidate)> = Vec::new();
    for pack in packs {
        for (i, d) in pack.header.entities.iter().enumerate() {
            let keys = d
                .tags
                .iter()
                .map(String::as_str)
                .chain([d.native_name.as_str(), d.name.as_str()])
                .chain(d.qualified.as_deref());
            let s = keys
                .filter_map(score)
                .min()
                .or_else(|| (q.len() >= 3 && d.brief.to_lowercase().contains(&q)).then_some(4));
            if let Some(s) = s {
                let c = out::entity_candidate(pack, i);
                hits.push((s, usize::from(d.tags.is_empty()), c.address.clone(), c));
            }
            let e = pack.entity(i);
            let mut push = |s: usize, hit: Hit| {
                let c = out::member_candidate(pack, i, &hit);
                hits.push((s + 1, 2, c.address.clone(), c));
            };
            for p in &e.properties {
                if let Some(s) = score(&p.js_name).into_iter().chain(score(&p.name)).min() {
                    push(
                        s,
                        Hit {
                            kind: MemberKind::Property,
                            owner: i,
                            member: Member::Property(p),
                        },
                    );
                }
            }
            for v in &e.events {
                if let Some(s) = [&v.js_event, &v.name]
                    .into_iter()
                    .filter_map(|n| score(n))
                    .min()
                {
                    push(
                        s,
                        Hit {
                            kind: MemberKind::Event,
                            owner: i,
                            member: Member::Event(v),
                        },
                    );
                }
            }
            let mut seen = std::collections::HashSet::new();
            for m in &e.methods {
                if seen.insert(&m.name)
                    && let Some(s) = score(&m.name)
                {
                    let all = e.methods.iter().filter(|x| x.name == m.name).collect();
                    push(
                        s,
                        Hit {
                            kind: MemberKind::Method,
                            owner: i,
                            member: Member::Method(all),
                        },
                    );
                }
            }
            for v in &e.values {
                if let Some(s) = score(&v.name) {
                    push(
                        s,
                        Hit {
                            kind: MemberKind::Value,
                            owner: i,
                            member: Member::Value(v),
                        },
                    );
                }
            }
            for sl in e.children.iter().flat_map(|c| &c.slots) {
                if let Some(s) = score(&sl.name) {
                    push(
                        s,
                        Hit {
                            kind: MemberKind::Slot,
                            owner: i,
                            member: Member::Slot(sl),
                        },
                    );
                }
            }
        }
    }
    hits.sort_by(|a, b| (a.0, a.1, &a.2).cmp(&(b.0, b.1, &b.2)));
    let truncated = hits.len() > limit;
    (
        hits.into_iter().take(limit).map(|h| h.3).collect(),
        truncated,
    )
}

fn apply_section(card: &mut out::Card, s: &str) -> Result<(), String> {
    let keep = match s {
        "props" | "properties" => "properties",
        "events" => "events",
        "slots" | "children" => "slots",
        "methods" => "methods",
        "ctors" | "constructors" => "constructors",
        "values" => "values",
        "dangerous" => "dangerous",
        "examples" => "examples",
        "description" => "description",
        _ => return Err(format!("unknown section {s:?}")),
    };
    if keep != "properties" {
        card.properties.clear();
    }
    if keep != "events" {
        card.events.clear();
    }
    if keep != "slots" {
        card.slots.clear();
        card.children_policy = None;
    }
    if keep != "methods" {
        card.methods.clear();
    }
    if keep != "constructors" {
        card.constructors.clear();
    }
    if keep != "values" {
        card.values.clear();
    }
    if keep != "dangerous" {
        card.dangerous.clear();
    }
    if keep != "examples" {
        card.examples.clear();
    }
    if keep != "description" {
        card.entity.description = None;
    }
    card.inherited.clear();
    card.entity.extensions.clear();
    Ok(())
}

pub fn run_docs(args: &[String], ctx: &Ctx) -> Output {
    let o = match parse(args) {
        Ok(Some(o)) => o,
        Ok(None) => {
            return Output {
                code: 0,
                stdout: DOCS_HELP.into(),
                stderr: String::new(),
            };
        }
        Err(e) => return usage(e),
    };
    let mut packs_arg = o.packs.clone();
    if packs_arg.is_empty() && o.irs.is_empty() {
        packs_arg = ctx.env_packs.clone();
    }
    let loaded = locate::load(&Search {
        cwd: ctx.cwd.clone(),
        packs: packs_arg,
        irs: o.irs.clone(),
        explicit_only: ctx.explicit_only,
    });
    let mut notes: Vec<String> = loaded.warnings.clone();
    let packs = loaded.packs;
    let st = Style {
        color: ctx.color && !o.json,
        brief: o.brief,
    };
    let project = locate::project_toolkit(&ctx.cwd);

    let finish = |code: i32, body: Body, notes: Vec<String>| -> Output {
        if o.json {
            let env = Envelope {
                schema: out::SCHEMA,
                body,
                notes,
            };
            let mut s = serde_json::to_string_pretty(&env).expect("serializable");
            s.push('\n');
            Output {
                code,
                stdout: s,
                stderr: String::new(),
            }
        } else {
            let stderr = notes.iter().map(|n| format!("note: {n}\n")).collect();
            Output {
                code,
                stdout: md::render(&st, &body),
                stderr,
            }
        }
    };

    if packs.is_empty() {
        let msg = "no docs packs found. In the svelters repo run `pnpm docs:ir` (the CLI builds the pack from the IR); \
                   elsewhere pass --pack <file> or set SVELTERS_DOCS_PACK.";
        notes.push(msg.into());
        if o.positional.is_empty() && o.search.is_none() {
            return finish(
                EXIT_NO_DATA,
                Body::Toolkits {
                    project: project.map(proj),
                    toolkits: vec![],
                },
                notes,
            );
        }
        return Output {
            code: EXIT_NO_DATA,
            stdout: String::new(),
            stderr: notes
                .iter()
                .map(|n| format!("svelters docs: {n}\n"))
                .collect(),
        };
    }
    let by_id = |id: &str| packs.iter().find(|p| p.toolkit().eq_ignore_ascii_case(id));
    let ids: Vec<String> = packs.iter().map(|p| p.toolkit().to_string()).collect();

    // toolkit: -t, else the first word when it names a toolkit
    let mut pos = o.positional.clone();
    let mut explicit = o.toolkit.clone();
    if explicit.is_none() && !pos.is_empty() && (pos.len() == 2 || by_id(&pos[0]).is_some()) {
        explicit = Some(pos.remove(0));
    }
    if let Some(t) = &explicit
        && by_id(t).is_none()
    {
        return usage(format!(
            "unknown toolkit {t:?} (available: {})",
            ids.join(", ")
        ));
    }
    if let (Some(t), Some((p, path))) = (&explicit, &project)
        && !t.eq_ignore_ascii_case(p)
    {
        notes.push(format!("this app targets {p} ({})", path.display()));
    }
    let scope: Vec<&Pack> = match (&explicit, &project) {
        (Some(t), _) => vec![by_id(t).unwrap()],
        (None, Some((p, path))) => match by_id(p) {
            Some(pk) => vec![pk],
            None => {
                notes.push(format!(
                    "{} names toolkit {p:?}, which has no docs pack; searching all",
                    path.display()
                ));
                packs.iter().collect()
            }
        },
        (None, None) => packs.iter().collect(),
    };
    let scope_ids: Vec<String> = scope.iter().map(|p| p.toolkit().to_string()).collect();

    if let Some(q) = &o.search {
        if !pos.is_empty() {
            return usage("-k takes the query as its value; drop the other arguments".into());
        }
        let (results, truncated) = search(&scope, q, o.limit);
        return finish(
            EXIT_OK,
            Body::Search {
                query: q.clone(),
                toolkits: scope_ids,
                results,
                truncated,
            },
            notes,
        );
    }

    let Some(addr) = pos.first().cloned() else {
        if let Some(t) = &explicit {
            let pk = by_id(t).unwrap();
            let rows: Vec<usize> = (0..pk.len()).collect();
            let elements = rows
                .iter()
                .filter(|&&i| !pk.header.entities[i].tags.is_empty())
                .map(|&i| out::index_row(pk, i))
                .collect();
            let all_types: Vec<usize> = rows
                .into_iter()
                .filter(|&i| pk.header.entities[i].tags.is_empty())
                .collect();
            let types = if o.all {
                all_types.iter().map(|&i| out::index_row(pk, i)).collect()
            } else {
                Vec::new()
            };
            return finish(
                EXIT_OK,
                Body::Index {
                    toolkit: Box::new(out::toolkit_out(pk)),
                    elements,
                    types,
                    types_total: all_types.len(),
                },
                notes,
            );
        }
        let toolkits = packs.iter().map(out::toolkit_out).collect();
        return finish(
            EXIT_OK,
            Body::Toolkits {
                project: project.map(proj),
                toolkits,
            },
            notes,
        );
    };

    let mut found: Vec<(&Pack, Res)> = Vec::new();
    let mut misses: Vec<(&Pack, Res)> = Vec::new();
    for pk in &scope {
        let r = resolve_in(pk, &addr);
        match r {
            Res::EntityMiss | Res::MemberMiss(..) => misses.push((pk, r)),
            _ => found.push((pk, r)),
        }
    }
    if found.len() > 1 {
        let mut candidates = Vec::new();
        for (pk, r) in &found {
            match r {
                Res::Entity(i) => candidates.push(out::entity_candidate(pk, *i)),
                Res::Member(i, h) => candidates.push(out::member_candidate(pk, *i, h)),
                Res::Ambiguous(c) => candidates.extend(c.iter().cloned()),
                _ => {}
            }
        }
        notes.push(format!(
            "`{addr}` exists in {}; name a toolkit (apps target one toolkit)",
            found
                .iter()
                .map(|(p, _)| p.toolkit())
                .collect::<Vec<_>>()
                .join(" and ")
        ));
        return finish(
            EXIT_AMBIGUOUS,
            Body::Ambiguous {
                query: addr,
                candidates,
            },
            notes,
        );
    }
    if let Some((pk, r)) = found.pop() {
        if explicit.is_none() && project.is_none() && scope.len() > 1 {
            notes.push(format!("found only in {}", pk.toolkit()));
        }
        let tk_name = pk.header.toolkit.name.clone();
        let data_version = pk.header.data_version.clone();
        let notices = pk.header.toolkit.notices.clone();
        return match r {
            Res::Entity(i) => {
                let mut card = out::card(pk, i, o.brief, o.all);
                if let Some(s) = &o.section
                    && let Err(e) = apply_section(&mut card, s)
                {
                    return usage(e);
                }
                if o.json {
                    finish(
                        EXIT_OK,
                        Body::Entity {
                            toolkit: pk.toolkit().into(),
                            data_version,
                            card: Box::new(card),
                            notices,
                        },
                        notes,
                    )
                } else {
                    // the markdown card names the toolkit by its display name
                    finish(
                        EXIT_OK,
                        Body::Entity {
                            toolkit: tk_name,
                            data_version,
                            card: Box::new(card),
                            notices,
                        },
                        notes,
                    )
                }
            }
            Res::Member(i, h) => {
                let member = Box::new(out::member_out(pk, i, &h, o.brief));
                finish(
                    EXIT_OK,
                    Body::Member {
                        toolkit: pk.toolkit().into(),
                        data_version,
                        entity: out::entity_ref(pk, i),
                        member,
                        notices,
                    },
                    notes,
                )
            }
            Res::Ambiguous(candidates) => finish(
                EXIT_AMBIGUOUS,
                Body::Ambiguous {
                    query: addr,
                    candidates,
                },
                notes,
            ),
            _ => unreachable!(),
        };
    }
    // not found anywhere: near names, then search results
    let mut suggestions: Vec<Candidate> = Vec::new();
    for (pk, r) in &misses {
        match r {
            Res::MemberMiss(i, q) => {
                for h in resolve::suggest_members(pk, *i, q, 6) {
                    suggestions.push(out::member_candidate(pk, *i, &h));
                }
            }
            _ => {
                let head = addr.split(['.']).next().unwrap_or(&addr);
                for (_, i) in resolve::suggest_entities(pk, head, 6) {
                    suggestions.push(out::entity_candidate(pk, i));
                }
            }
        }
    }
    if suggestions.len() < 6 && misses.iter().all(|(_, r)| matches!(r, Res::EntityMiss)) {
        let (more, _) = search(&scope, addr.split('.').next().unwrap_or(&addr), 6);
        for c in more {
            if !suggestions.iter().any(|s| s.address == c.address) && suggestions.len() < 8 {
                suggestions.push(c);
            }
        }
    }
    finish(
        EXIT_NOT_FOUND,
        Body::NotFound {
            query: addr,
            toolkits: scope_ids,
            suggestions,
        },
        notes,
    )
}

fn proj((t, p): (String, PathBuf)) -> ProjectOut {
    ProjectOut {
        toolkit: t,
        config: p.display().to_string(),
    }
}

pub fn run_pack(args: &[String], ctx: &Ctx) -> Output {
    let mut ir: Option<PathBuf> = None;
    let mut outdir: Option<PathBuf> = None;
    let mut check = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                return Output {
                    code: 0,
                    stdout: PACK_HELP.into(),
                    stderr: String::new(),
                };
            }
            "--ir" => ir = it.next().map(PathBuf::from),
            "--out" => outdir = it.next().map(PathBuf::from),
            "--check" => check = true,
            _ => {
                return Output {
                    code: EXIT_USAGE,
                    stdout: String::new(),
                    stderr: format!("svelters docs-pack: unknown argument {a}\n{PACK_HELP}"),
                };
            }
        }
    }
    let root = locate::repo_root(&ctx.cwd).or_else(|| {
        ctx.cwd
            .ancestors()
            .find(|d| d.join("tools/docs/build_ir.py").is_file())
            .map(Into::into)
    });
    let ir = match ir
        .map(|p| ctx.cwd.join(p))
        .or_else(|| root.as_ref().map(|r| r.join("docs/site/.generated/ir")))
    {
        Some(p) => p,
        None => {
            return Output {
                code: EXIT_USAGE,
                stdout: String::new(),
                stderr: "svelters docs-pack: not in the repo; pass --ir\n".into(),
            };
        }
    };
    let outdir = outdir
        .map(|p| ctx.cwd.join(p))
        .or_else(|| root.as_ref().map(|r| r.join("docs/site/.generated/pack")))
        .unwrap_or_else(|| ctx.cwd.clone());
    let files: Vec<PathBuf> = if ir.is_dir() {
        let mut v: Vec<PathBuf> = std::fs::read_dir(&ir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        v.sort();
        v
    } else {
        vec![ir.clone()]
    };
    if files.is_empty() {
        return Output {
            code: EXIT_NO_DATA,
            stdout: String::new(),
            stderr: format!(
                "svelters docs-pack: no IR in {} (run `pnpm docs:ir`)\n",
                ir.display()
            ),
        };
    }
    let mut stdout = String::new();
    let mut code = 0;
    for f in files {
        let t0 = std::time::Instant::now();
        let bytes = match std::fs::read(&f) {
            Ok(b) => b,
            Err(e) => {
                return Output {
                    code: EXIT_NO_DATA,
                    stdout,
                    stderr: format!("{}: {e}\n", f.display()),
                };
            }
        };
        let pack = match crate::pack::build(&bytes) {
            Ok(p) => p,
            Err(e) => {
                return Output {
                    code: EXIT_NO_DATA,
                    stdout,
                    stderr: format!("{}: {e}\n", f.display()),
                };
            }
        };
        let ms = t0.elapsed().as_millis();
        let stem = f.file_stem().unwrap().to_string_lossy();
        let dest = outdir.join(format!("{stem}.svpack"));
        if check {
            match std::fs::read(&dest) {
                Ok(old) if old == pack => {
                    stdout.push_str(&format!("{}: up to date\n", dest.display()))
                }
                _ => {
                    stdout.push_str(&format!("{}: stale or missing\n", dest.display()));
                    code = 1;
                }
            }
            continue;
        }
        if let Err(e) = std::fs::create_dir_all(&outdir).and_then(|_| std::fs::write(&dest, &pack))
        {
            return Output {
                code: EXIT_NO_DATA,
                stdout,
                stderr: format!("{}: {e}\n", dest.display()),
            };
        }
        stdout.push_str(&format!(
            "{}: {} bytes from {} bytes of IR in {ms} ms\n",
            dest.display(),
            pack.len(),
            bytes.len()
        ));
    }
    Output {
        code,
        stdout,
        stderr: String::new(),
    }
}
