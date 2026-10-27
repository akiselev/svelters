//! The `skelter-gallery` CLI ([`cli_main`]): a host's binary is `fn main() -> ExitCode {
//! skelter_gallery::cli_main(Extensions::new().story_root(..)) }`. The JSON outputs are the contract
//! agents use (`docs/skelter/gallery.md`). Exit codes: 0 ok, 1 a story failed, 2 bad arguments.

use crate::catalog::{Catalog, discover_with};
use crate::ext::Extensions;
use crate::matrix::{Axes, Matrix};
use crate::model::{Cell, State, Story, density_named, parse_arg_value};
use crate::render::{Error, Renderer, save_png};
use crate::sheet::{Sheet, SheetLayout};
use serde_json::json;
use skelter_core::KindRegistry;
use skelter_widgets::theme::Density;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
usage: skelter-gallery [--root <gallery dir>].. [--base-css <file>] <command>

commands:
  list [--json]                              stories, variants, kinds and coverage gaps
  shot <story>[/<variant>] [cell flags] [-o out.png] [--json]
                                             one cell as a PNG
  sheet (<story> | --family <f> | --all) [axis flags] [-o out.png|dir] [--index out.json] [--json]
                                             a contact sheet: rows variant × args × state, columns theme × zoom × density
  inspect <story>/<variant> <selector> [cell flags] [--json]
                                             one node of a cell: kind, anchor, box, computed style, matched rules, forced
                                             state, AccessKit node (selector: #anchor or a kind name)
  check [--json]                             every variant parses, compiles and renders one cell; exit 1 on any error

cell flags:  --args k=v,..  --state <s>  --theme <t>  --density <d>  --zoom <z>
axis flags:  --variants a,b  --args k=v,..  --states s,..  --themes t,..  --densities d,..  --zooms z,..

states: normal hover active focus focus-visible checked disabled
themes: dark-modern light-modern hc-dark hc-light (and a host's)
densities: compact default comfortable
Output goes where -o says; the default is target/gallery/.";

/// Exit codes.
const OK: u8 = 0;
const FAILED: u8 = 1;
const BAD_ARGS: u8 = 2;

/// A bad command line.
struct Usage(String);

impl From<String> for Usage {
    fn from(s: String) -> Usage {
        Usage(s)
    }
}

/// The command line after the global flags.
pub(crate) struct Args {
    pub(crate) positional: Vec<String>,
    pub(crate) flags: BTreeMap<String, String>,
    pub(crate) switches: Vec<String>,
}

const VALUED: &[&str] = &[
    "--root", "--base-css", "--args", "--state", "--theme", "--density", "--zoom", "-o", "--out", "--index", "--family", "--variants", "--states", "--themes", "--densities", "--zooms",
];
const SWITCHES: &[&str] = &["--json", "--all", "-h", "--help"];

fn parse(argv: &[String]) -> Result<(Vec<PathBuf>, Args), Usage> {
    let mut roots = Vec::new();
    let mut a = Args { positional: Vec::new(), flags: BTreeMap::new(), switches: Vec::new() };
    let mut it = argv.iter();
    while let Some(s) = it.next() {
        if VALUED.contains(&s.as_str()) {
            let v = it.next().ok_or_else(|| Usage(format!("{s} needs a value")))?;
            if s == "--root" {
                roots.push(PathBuf::from(v));
            } else {
                let k = if s == "--out" { "-o".to_string() } else { s.clone() };
                if a.flags.insert(k, v.clone()).is_some() {
                    return Err(Usage(format!("{s} given twice")));
                }
            }
        } else if SWITCHES.contains(&s.as_str()) {
            a.switches.push(s.clone());
        } else if s.starts_with('-') && s.len() > 1 {
            return Err(Usage(format!("unknown flag {s}")));
        } else {
            a.positional.push(s.clone());
        }
    }
    Ok((roots, a))
}

impl Args {
    fn switch(&self, s: &str) -> bool {
        self.switches.iter().any(|x| x == s)
    }

    fn flag(&self, k: &str) -> Option<&str> {
        self.flags.get(k).map(String::as_str)
    }

    /// Fails on a flag the command doesn't take.
    fn only(&self, cmd: &str, allowed: &[&str]) -> Result<(), Usage> {
        for k in self.flags.keys().chain(self.switches.iter()) {
            if !allowed.contains(&k.as_str()) && k != "--base-css" {
                return Err(Usage(format!("{cmd} takes no {k}")));
            }
        }
        Ok(())
    }
}

/// `k=v,k2=v2` (values JSON when they parse, else strings).
fn parse_kv(s: &str) -> Result<BTreeMap<String, serde_json::Value>, Usage> {
    let mut out = BTreeMap::new();
    for part in s.split(',').filter(|p| !p.is_empty()) {
        let (k, v) = part.split_once('=').ok_or_else(|| Usage(format!("--args: {part:?} is not k=v")))?;
        out.insert(k.trim().to_string(), parse_arg_value(v.trim()));
    }
    Ok(out)
}

fn list_of<T>(s: &str, what: &str, f: impl Fn(&str) -> Option<T>) -> Result<Vec<T>, Usage> {
    s.split(',').filter(|p| !p.is_empty()).map(|p| f(p.trim()).ok_or_else(|| Usage(format!("{p:?} is not a {what}")))).collect()
}

fn zoom(s: &str) -> Option<f32> {
    s.parse::<f32>().ok().filter(|z| (0.25..=8.0).contains(z))
}

/// The CLI on `ext` (see the module docs).
pub fn cli_main(ext: Extensions) -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(run(ext, &argv))
}

/// [`cli_main`] on `argv` (the arguments after the program name): the exit code.
pub fn run(mut ext: Extensions, argv: &[String]) -> u8 {
    let (roots, args) = match parse(argv) {
        Ok(x) => x,
        Err(Usage(m)) => {
            eprintln!("skelter-gallery: {m}\n\n{USAGE}");
            return BAD_ARGS;
        }
    };
    if args.switch("-h") || args.switch("--help") || args.positional.is_empty() {
        println!("{USAGE}");
        return if args.positional.is_empty() && !(args.switch("-h") || args.switch("--help")) { BAD_ARGS } else { OK };
    }
    ext.story_roots.extend(roots);
    if ext.story_roots.is_empty() {
        match default_root() {
            Some(r) => ext.story_roots.push(r),
            None => {
                eprintln!("skelter-gallery: no gallery root: pass --root <dir> (a directory with stories/), or run inside the svelters checkout");
                return BAD_ARGS;
            }
        }
    }
    let cmd = args.positional[0].clone();
    let rest = Args { positional: args.positional[1..].to_vec(), flags: args.flags, switches: args.switches };
    let r = match cmd.as_str() {
        "list" => cmd_list(&ext, &rest),
        "shot" => cmd_shot(&ext, &rest),
        "sheet" => cmd_sheet(&ext, &rest),
        "inspect" => cmd_inspect(&ext, &rest),
        _ => Err(Fail::Usage(format!("unknown command {cmd:?}"))),
    };
    match r {
        Ok(code) => code,
        Err(Fail::Usage(m)) => {
            eprintln!("skelter-gallery: {m} (skelter-gallery --help)");
            BAD_ARGS
        }
        Err(Fail::Error(e)) => {
            eprintln!("skelter-gallery: {e}");
            match e {
                Error::Request(_) => BAD_ARGS,
                _ => FAILED,
            }
        }
    }
}

pub(crate) enum Fail {
    Usage(String),
    Error(Error),
}

impl From<Usage> for Fail {
    fn from(u: Usage) -> Fail {
        Fail::Usage(u.0)
    }
}

impl From<Error> for Fail {
    fn from(e: Error) -> Fail {
        Fail::Error(e)
    }
}

/// `gallery/` in the current directory or the nearest one above it that has one.
fn default_root() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    cwd.ancestors().map(|d| d.join("gallery")).find(|g| g.join("stories").is_dir())
}

/// Where outputs go by default: `target/gallery/` next to the gallery root's checkout, else in the
/// current directory.
fn out_dir(ext: &Extensions) -> PathBuf {
    let workspace = |d: &Path| std::fs::read_to_string(d.join("Cargo.toml")).is_ok_and(|t| t.contains("[workspace]"));
    let base = ext.story_roots.first().and_then(|r| r.canonicalize().ok()).and_then(|r| r.ancestors().find(|d| workspace(d)).map(Path::to_path_buf));
    base.unwrap_or_else(|| PathBuf::from(".")).join("target/gallery")
}

/// skelter-widgets' `base.css` source, when the gallery runs inside the svelters checkout: edits to
/// it show without a rebuild.
fn base_css_source(ext: &Extensions) -> Option<PathBuf> {
    ext.story_roots.iter().filter_map(|r| r.canonicalize().ok()).find_map(|r| r.ancestors().map(|d| d.join("crates/skelter-widgets/src/theme/base.css")).find(|p| p.is_file()))
}

fn catalog(ext: &Extensions) -> Result<Catalog, Fail> {
    discover_with(&ext.story_roots, &ext.families).map_err(|e| Fail::Error(Error::Stories(e)))
}

pub(crate) fn renderer(ext: &Extensions, args: &Args) -> Result<Renderer, Fail> {
    let mut r = Renderer::new(ext)?;
    let css = match args.flag("--base-css") {
        Some(f) => Some(PathBuf::from(f)),
        None => base_css_source(ext),
    };
    if let Some(f) = css {
        let text = std::fs::read_to_string(&f).map_err(|e| Fail::Usage(format!("--base-css {}: {e}", f.display())))?;
        r.set_base_css(text)?;
    }
    Ok(r)
}

/// The kinds the gallery registers: the built-ins and the host's.
pub fn registry(ext: &Extensions) -> Result<KindRegistry, String> {
    let mut reg = KindRegistry::new();
    svelters_skelter::kinds::register_builtin(&mut reg).map_err(|e| e.to_string())?;
    for f in &ext.register_kinds {
        f(&mut reg)?;
    }
    Ok(reg)
}

fn print_json(v: &serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(v).expect("JSON"));
}

// ---- list ----

fn cmd_list(ext: &Extensions, a: &Args) -> Result<u8, Fail> {
    a.only("list", &["--json"])?;
    if !a.positional.is_empty() {
        return Err(Fail::Usage("list takes no arguments".into()));
    }
    let cat = catalog(ext)?;
    let reg = registry(ext).map_err(|e| Fail::Error(Error::Setup(e)))?;
    let cov = cat.coverage(&reg);
    let kinds: Vec<&str> = {
        let mut k: Vec<&str> = reg.iter().map(|e| e.spec.name).collect();
        k.sort();
        k
    };
    if a.switch("--json") {
        print_json(&json!({
            "roots": ext.story_roots,
            "stories": cat.stories().iter().map(story_json).collect::<Vec<_>>(),
            "kinds": kinds,
            "coverage": cov,
            "themes": ext.theme_names(),
            "densities": Density::ALL.map(|d| d.name()),
            "states": State::ALL.map(|s| s.name()),
        }));
        return Ok(OK);
    }
    for s in cat.stories() {
        println!("{}/{}  {}  [{}]", s.family, s.id, s.title, s.kinds.join(", "));
        for v in &s.variants {
            let args = if v.args.is_empty() { String::new() } else { format!("  args: {}", v.args.iter().map(|(k, vs)| format!("{k}={}", vs.iter().map(crate::model::value_label).collect::<Vec<_>>().join("|"))).collect::<Vec<_>>().join(" ")) };
            println!("    {}  {}{args}", v.id, v.title);
        }
    }
    println!("\n{} stories, {} kinds registered, {} covered", cat.stories().len(), kinds.len(), cov.covered.len());
    if !cov.missing.is_empty() {
        println!("kinds with no story ({}): {}", cov.missing.len(), cov.missing.join(", "));
    }
    for (s, k) in &cov.unknown {
        println!("story {s} names {k}, which the registry lacks");
    }
    Ok(OK)
}

pub(crate) fn story_json(s: &Story) -> serde_json::Value {
    let mut v = serde_json::to_value(s).expect("JSON");
    v["manifest"] = json!(s.manifest());
    v
}

// ---- shot ----

/// `<story>[/<variant>]` and the cell flags as a cell.
pub(crate) fn cell_of(r: &Renderer, ext: &Extensions, target: &str, a: &Args) -> Result<Cell, Fail> {
    let (story, variant) = match target.split_once('/') {
        Some((s, v)) => (s, Some(v)),
        None => (target, None),
    };
    let s = r.catalog().get(story).ok_or_else(|| Fail::Usage(format!("no story {story:?}")))?;
    let v = match variant {
        Some(v) => s.variant(v).ok_or_else(|| Fail::Usage(format!("story {story} has no variant {v:?} (it has {})", s.variants.iter().map(|v| v.id.0.as_str()).collect::<Vec<_>>().join(", "))))?,
        None => &s.variants[0],
    };
    let mut c = Cell::new(s, v);
    if let Some(x) = a.flag("--args") {
        for (k, val) in parse_kv(x)? {
            if !v.args.contains_key(&k) && !v.props.contains(&k) {
                return Err(Fail::Usage(format!("variant {}/{} has no arg or prop {k}", s.id, v.id)));
            }
            c.args.insert(k, val);
        }
    }
    if let Some(x) = a.flag("--state") {
        c.state = State::parse(x).ok_or_else(|| Fail::Usage(format!("{x:?} is not a state")))?;
        if !s.states.contains(&c.state) {
            return Err(Fail::Usage(format!("story {} does not show state {x} (it lists {})", s.id, s.states.iter().map(|s| s.name()).collect::<Vec<_>>().join(", "))));
        }
    }
    if let Some(x) = a.flag("--theme") {
        c.theme = ext.theme_named(x).ok_or_else(|| Fail::Usage(format!("{x:?} is not a theme ({})", ext.theme_names().join(", "))))?;
    }
    if let Some(x) = a.flag("--density") {
        c.density = density_named(x).ok_or_else(|| Fail::Usage(format!("{x:?} is not a density")))?;
    }
    if let Some(x) = a.flag("--zoom") {
        c.zoom = zoom(x).ok_or_else(|| Fail::Usage(format!("{x:?} is not a zoom from 0.25 to 8")))?;
    }
    Ok(c)
}

pub(crate) const CELL_FLAGS: &[&str] = &["--args", "--state", "--theme", "--density", "--zoom"];

/// A file name for a cell: `button-primary-size=small-hover-dark-modern.png`.
pub(crate) fn cell_file(c: &Cell) -> String {
    let mut parts = vec![c.story.0.clone(), c.variant.0.clone()];
    if !c.args.is_empty() {
        parts.push(c.args.iter().map(|(k, v)| format!("{k}={}", crate::model::value_label(v))).collect::<Vec<_>>().join(","));
    }
    parts.push(c.state.name().into());
    parts.push(c.theme.name().into());
    if c.density != Density::Default {
        parts.push(c.density.name().into());
    }
    if c.zoom != 1.0 {
        parts.push(format!("x{}", c.zoom));
    }
    let s = parts.join("-");
    s.chars().map(|c| if c.is_alphanumeric() || "-_=,.".contains(c) { c } else { '_' }).collect::<String>() + ".png"
}

fn cmd_shot(ext: &Extensions, a: &Args) -> Result<u8, Fail> {
    let mut allowed = CELL_FLAGS.to_vec();
    allowed.extend(["-o", "--json"]);
    a.only("shot", &allowed)?;
    let [target] = &a.positional[..] else {
        return Err(Fail::Usage("shot takes one <story>[/<variant>]".into()));
    };
    let mut r = renderer(ext, a)?;
    let cell = cell_of(&r, ext, target, a)?;
    let out = a.flag("-o").map(PathBuf::from).unwrap_or_else(|| out_dir(ext).join(cell_file(&cell)));
    let shot = r.shot(&cell)?;
    save_png(&shot.image, &out).map_err(|e| Fail::Error(Error::Render(format!("{}: {e}", out.display()))))?;
    if a.switch("--json") {
        print_json(&json!({
            "file": out,
            "cell": cell,
            "label": cell.label(),
            "size": [shot.image.width, shot.image.height],
            "applied": shot.applied,
            "warnings": shot.warnings,
        }));
    } else {
        println!("{}", out.display());
        if let Some(x) = &shot.applied {
            eprintln!("{}: {x}", cell.label());
        }
        for w in &shot.warnings {
            eprintln!("warning: {w}");
        }
    }
    Ok(OK)
}

/// A story's matrix from the axis flags.
pub(crate) fn matrix_of(ext: &Extensions, story: &Story, a: &Args, many: bool) -> Result<Matrix, Fail> {
    let mut axes = Axes::default();
    if let Some(x) = a.flag("--variants") {
        axes.variants = Some(x.split(',').filter(|p| !p.is_empty()).map(|p| p.trim().into()).collect());
    }
    if let Some(x) = a.flag("--args") {
        axes.args = parse_kv(x)?;
    }
    if let Some(x) = a.flag("--states") {
        let want = list_of(x, "state", State::parse)?;
        // a family or --all sheet shows each story's states among those asked
        axes.states = Some(if many { want.into_iter().filter(|s| story.states.contains(s)).collect() } else { want });
    }
    if let Some(x) = a.flag("--themes") {
        axes.themes = list_of(x, "theme", |t| ext.theme_named(t))?;
    }
    if let Some(x) = a.flag("--densities") {
        axes.densities = list_of(x, "density", density_named)?;
    }
    if let Some(x) = a.flag("--zooms") {
        axes.zooms = list_of(x, "zoom from 0.25 to 8", zoom)?;
    }
    Matrix::new(story, &axes).map_err(Fail::Usage)
}

// ---- sheet ----

fn cmd_sheet(ext: &Extensions, a: &Args) -> Result<u8, Fail> {
    a.only("sheet", &["--family", "--all", "--variants", "--args", "--states", "--themes", "--densities", "--zooms", "-o", "--index", "--json"])?;
    let family = a.flag("--family");
    let all = a.switch("--all");
    let modes = usize::from(family.is_some()) + usize::from(all) + usize::from(!a.positional.is_empty());
    if modes != 1 || a.positional.len() > 1 {
        return Err(Fail::Usage("sheet takes one of <story>, --family <f>, --all".into()));
    }
    let mut r = renderer(ext, a)?;
    let layout = SheetLayout::default();
    if let [story] = &a.positional[..] {
        let s = r.catalog().get(story).cloned().ok_or_else(|| Fail::Usage(format!("no story {story:?}")))?;
        let m = matrix_of(ext, &s, a, false)?;
        let out = a.flag("-o").map(PathBuf::from).unwrap_or_else(|| out_dir(ext).join(format!("{}.png", s.id)));
        let sheet = r.sheet(&m, layout)?;
        save_png(&sheet.image, &out).map_err(|e| Fail::Error(Error::Render(format!("{}: {e}", out.display()))))?;
        let index = sheet.index(&m, Some(&out));
        if let Some(f) = a.flag("--index") {
            write_json(Path::new(f), &index)?;
        }
        report(&sheet, &out);
        if a.switch("--json") {
            print_json(&index);
        } else {
            println!("{}", out.display());
        }
        return Ok(if sheet.errors().next().is_some() { FAILED } else { OK });
    }
    let stories: Vec<Story> = match family {
        Some(f) => {
            let v: Vec<Story> = r.catalog().family(f).cloned().collect();
            if v.is_empty() {
                return Err(Fail::Usage(format!("no stories in family {f:?}")));
            }
            v
        }
        None => r.catalog().stories().to_vec(),
    };
    let dir = a.flag("-o").map(PathBuf::from).unwrap_or_else(|| out_dir(ext).join("sheets").join(family.unwrap_or("all")));
    let mut sheets = Vec::new();
    let mut failed = false;
    let (mut cells, mut secs) = (0usize, 0f64);
    for s in &stories {
        let m = matrix_of(ext, s, a, true)?;
        if m.is_empty() {
            continue;
        }
        let out = dir.join(format!("{}.png", s.id));
        let sheet = r.sheet(&m, layout.clone())?;
        save_png(&sheet.image, &out).map_err(|e| Fail::Error(Error::Render(format!("{}: {e}", out.display()))))?;
        report(&sheet, &out);
        failed |= sheet.errors().next().is_some();
        cells += sheet.cells.len();
        secs += sheet.seconds;
        sheets.push(sheet.index(&m, Some(&out)));
        if !a.switch("--json") {
            println!("{}", out.display());
        }
    }
    let index = json!({ "sheets": sheets, "cells": cells, "seconds": secs });
    write_json(&dir.join("index.json"), &index)?;
    if let Some(f) = a.flag("--index") {
        write_json(Path::new(f), &index)?;
    }
    if a.switch("--json") {
        print_json(&index);
    } else {
        println!("{}", dir.join("index.json").display());
    }
    Ok(if failed { FAILED } else { OK })
}

/// A sheet's errors and speed on stderr.
fn report(sheet: &Sheet, out: &Path) {
    for c in sheet.errors() {
        eprintln!("{}: {}: {}", out.display(), c.cell.label(), c.error.as_deref().unwrap_or_default());
    }
    let n = sheet.cells.len();
    eprintln!("{}: {n} cells in {:.2} s ({:.1} cells/s)", out.display(), sheet.seconds, n as f64 / sheet.seconds.max(1e-9));
}

fn write_json(path: &Path, v: &serde_json::Value) -> Result<(), Fail> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| Fail::Error(Error::Render(format!("{}: {e}", d.display()))))?;
    }
    std::fs::write(path, serde_json::to_string_pretty(v).expect("JSON")).map_err(|e| Fail::Error(Error::Render(format!("{}: {e}", path.display()))))
}

// ---- inspect ----

fn cmd_inspect(ext: &Extensions, a: &Args) -> Result<u8, Fail> {
    let mut allowed = CELL_FLAGS.to_vec();
    allowed.push("--json");
    a.only("inspect", &allowed)?;
    let [target, selector] = &a.positional[..] else {
        return Err(Fail::Usage("inspect takes <story>/<variant> <selector>".into()));
    };
    let mut r = renderer(ext, a)?;
    let cell = cell_of(&r, ext, target, a)?;
    let v = r.inspect(&cell, selector)?;
    // JSON either way: it is the readable form too
    print_json(&v);
    Ok(OK)
}
