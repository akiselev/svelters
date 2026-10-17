//! Shared oracle harness (NC1, agent F): corpus loading, byte -> UTF-16 offset conversion,
//! normalization (ignore list), deep JSON diff, guarded execution, reporting, baseline ratchet.
//!
//! The corpus (target/oracle/{acorn,svelte}.jsonl) is produced by `tools/oracle/gen.mjs` from the
//! real acorn 8.19.0 / svelte/compiler 5.57.2. It is the definition of correctness: never edit it.
//!
//! ## Ignore list (applied to BOTH sides before comparing)
//!
//! | key | where | why |
//! | --- | --- | --- |
//! | `loc`, `range` | everywhere | derived from `start`/`end` (`diag::LineIndex`); not stored by `estree` |
//! | `Literal.value` when `regex` is present | everywhere | a host `RegExp` object: `{}` in JSON, or `null` when the Node running the generator rejects the pattern; `regex.{pattern,flags}` is compared |
//! | `metadata` | svelte | analysis data; the Rust AST keeps side tables instead (`svelte::ast` docs) |
//! | `name_loc` | svelte | reduced to a non-serialized span (`name_span`) in `svelte::ast` |
//! | `leadingComments`, `trailingComments` | svelte | attached to ESTree nodes by Svelte for printing; the Rust AST keeps only `Root.comments` |
//! | `Root.css.*` except `type,start,end,attributes,content.{start,end,styles}` | svelte | deviation: `StyleBlock` is the raw block, not Svelte's CSS AST (`crate::style` parses it) |
//! | `Root.js` when it is `[]` | svelte | legacy leftover, always empty in modern output (not in template.d.ts) |
//! | `Root.options.customElement` | svelte | accepted but unsupported; `SvelteOptions.custom_element` is deliberately not serialized |
//!
//! Everything else is compared exactly: key sets (a key absent on one side and `null` on the
//! other IS a difference), array lengths, strings, booleans. Numbers compare by value (`1` ==
//! `1.0`), since serde writes `f64` literal values with a fraction.
//!
//! Positions: our ASTs carry BYTE offsets; every number under a key named `start` or `end` (and
//! error positions) is converted to a UTF-16 offset with [`Utf16Map`] before comparing. A byte
//! offset that is not on a char boundary becomes a string `"<byte N: not a char boundary>"`, so it
//! always mismatches.
#![allow(dead_code)]

use serde_json::{Map, Value};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::Once;
use std::time::Instant;

// ---------------------------------------------------------------------------------------------
// paths

pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `$ORACLE_DIR` or `<workspace>/target/oracle`.
pub fn oracle_dir() -> PathBuf {
    std::env::var_os("ORACLE_DIR").map(PathBuf::from).unwrap_or_else(|| workspace_root().join("target/oracle"))
}

pub fn baseline_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/oracle_baseline.json")
}

/// Loads `<oracle_dir>/<suite>.jsonl`; `None` (with a printed explanation) if it does not exist.
pub fn load_corpus(suite: &str) -> Option<Vec<Value>> {
    let path = oracle_dir().join(format!("{suite}.jsonl"));
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => {
            println!(
                "oracle[{suite}]: SKIPPED, corpus {} not found.\n  Generate it with `tools/oracle/run.sh` \
                 (or `pnpm install --frozen-lockfile && node tools/oracle/gen.mjs {suite}`); \
                 needs upstream checkouts in $UPSTREAM (default /tmp/claude-0/up).",
                path.display()
            );
            return None;
        }
    };
    let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    let cases = parallel_map(&lines, |l| serde_json::from_str::<Value>(l).expect("corpus line is not valid JSON"));
    Some(cases)
}

// ---------------------------------------------------------------------------------------------
// offsets

const NOT_BOUNDARY: u32 = u32::MAX;

/// Byte offset -> UTF-16 offset for one source text.
pub struct Utf16Map {
    table: Vec<u32>,
}

impl Utf16Map {
    pub fn new(src: &str) -> Utf16Map {
        let mut table = vec![NOT_BOUNDARY; src.len() + 1];
        let mut u = 0u32;
        for (b, ch) in src.char_indices() {
            table[b] = u;
            u += ch.len_utf16() as u32;
        }
        table[src.len()] = u;
        Utf16Map { table }
    }

    pub fn to_utf16(&self, byte: u64) -> Option<u64> {
        match self.table.get(byte as usize) {
            Some(&u) if u != NOT_BOUNDARY => Some(u as u64),
            _ => None,
        }
    }

    pub fn convert(&self, byte: u64) -> Value {
        match self.to_utf16(byte) {
            Some(u) => Value::from(u),
            None => Value::String(format!("<byte {byte}: not a char boundary>")),
        }
    }
}

/// UTF-16 offset -> byte offset (inverse of [`Utf16Map`]; used by the self-tests only).
pub struct ByteMap {
    table: Vec<u32>,
}

impl ByteMap {
    pub fn new(src: &str) -> ByteMap {
        let units: usize = src.encode_utf16().count();
        let mut table = vec![NOT_BOUNDARY; units + 1];
        let mut u = 0usize;
        for (b, ch) in src.char_indices() {
            table[u] = b as u32;
            u += ch.len_utf16();
        }
        table[units] = src.len() as u32;
        ByteMap { table }
    }

    pub fn to_byte(&self, u: u64) -> Option<u64> {
        match self.table.get(u as usize) {
            Some(&b) if b != NOT_BOUNDARY => Some(b as u64),
            _ => None,
        }
    }
}

/// Rewrites every number under a `start`/`end` key with `f`.
pub fn map_offsets(v: &mut Value, f: &dyn Fn(u64) -> Value) {
    match v {
        Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                if (k == "start" || k == "end") && x.is_u64() {
                    *x = f(x.as_u64().unwrap());
                } else {
                    map_offsets(x, f);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| map_offsets(x, f)),
        _ => {}
    }
}

/// Our side: byte offsets -> UTF-16.
pub fn bytes_to_utf16(v: &mut Value, map: &Utf16Map) {
    map_offsets(v, &|b| map.convert(b));
}

// ---------------------------------------------------------------------------------------------
// normalization

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Suite {
    Acorn,
    Svelte,
}

impl Suite {
    pub fn name(self) -> &'static str {
        match self {
            Suite::Acorn => "acorn",
            Suite::Svelte => "svelte",
        }
    }
}

const IGNORE_ALWAYS: &[&str] = &["loc", "range"];
const IGNORE_SVELTE: &[&str] = &["metadata", "name_loc", "leadingComments", "trailingComments"];

fn strip(v: &mut Value, suite: Suite) {
    match v {
        Value::Object(m) => {
            m.retain(|k, _| !IGNORE_ALWAYS.contains(&k.as_str()) && !(suite == Suite::Svelte && IGNORE_SVELTE.contains(&k.as_str())));
            if m.get("type").and_then(Value::as_str) == Some("Literal") && m.contains_key("regex") {
                m.remove("value");
            }
            m.values_mut().for_each(|x| strip(x, suite));
        }
        Value::Array(a) => a.iter_mut().for_each(|x| strip(x, suite)),
        _ => {}
    }
}

/// Applies the ignore list (module docs) in place. `v` is an acorn `Program` (or the
/// `{ast, comments}` wrapper) for `Suite::Acorn`, a svelte `Root` for `Suite::Svelte`.
pub fn normalize(v: &mut Value, suite: Suite) {
    strip(v, suite);
    if suite == Suite::Svelte {
        if let Some(root) = v.as_object_mut() {
            if root.get("js").is_some_and(|j| j.as_array().is_some_and(Vec::is_empty)) {
                root.remove("js");
            }
            if let Some(Value::Object(css)) = root.get_mut("css") {
                css.retain(|k, _| matches!(k.as_str(), "type" | "start" | "end" | "attributes" | "content"));
                if let Some(Value::Object(content)) = css.get_mut("content") {
                    content.retain(|k, _| matches!(k.as_str(), "start" | "end" | "styles"));
                }
            }
            if let Some(Value::Object(opts)) = root.get_mut("options") {
                opts.remove("customElement");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// diff

#[derive(Debug, Clone)]
pub struct Diff {
    pub path: String,
    pub expected: String,
    pub ours: String,
}

impl std::fmt::Display for Diff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "at {}: expected {} | ours {}", self.path, self.expected, self.ours)
    }
}

pub fn short(v: Option<&Value>) -> String {
    match v {
        None => "<absent>".into(),
        Some(v) => {
            let s = v.to_string();
            if s.chars().count() > 160 {
                format!("{}...", s.chars().take(160).collect::<String>())
            } else {
                s
            }
        }
    }
}

fn num_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x == y || x.as_f64() == y.as_f64(),
        _ => false,
    }
}

/// First difference between `expected` and `ours`, depth first in expected's key order (then
/// keys only ours has). Paths look like `body[3].expression.callee.name`; when an object has a
/// `type`, it is shown in the path (`body[3]<ExpressionStatement>.expression`).
pub fn diff(expected: &Value, ours: &Value) -> Option<Diff> {
    let mut path = String::new();
    diff_at(expected, ours, &mut path)
}

fn diff_at(e: &Value, o: &Value, path: &mut String) -> Option<Diff> {
    let mk = |path: &str, e: Option<&Value>, o: Option<&Value>| Diff {
        path: if path.is_empty() { "<root>".into() } else { path.to_string() },
        expected: short(e),
        ours: short(o),
    };
    match (e, o) {
        (Value::Object(em), Value::Object(om)) => {
            let len = path.len();
            if let Some(t) = em.get("type").and_then(Value::as_str) {
                if om.get("type").and_then(Value::as_str) == Some(t) {
                    let _ = write!(path, "<{t}>");
                }
            }
            for (k, ev) in em {
                let l2 = path.len();
                if !path.is_empty() {
                    path.push('.');
                }
                path.push_str(k);
                match om.get(k) {
                    None => return Some(mk(path, Some(ev), None)),
                    Some(ov) => {
                        if let Some(d) = diff_at(ev, ov, path) {
                            return Some(d);
                        }
                    }
                }
                path.truncate(l2);
            }
            for (k, ov) in om {
                if !em.contains_key(k) {
                    if !path.is_empty() {
                        path.push('.');
                    }
                    path.push_str(k);
                    return Some(mk(path, None, Some(ov)));
                }
            }
            path.truncate(len);
            None
        }
        (Value::Array(ea), Value::Array(oa)) => {
            for (i, (ev, ov)) in ea.iter().zip(oa).enumerate() {
                let l2 = path.len();
                let _ = write!(path, "[{i}]");
                if let Some(d) = diff_at(ev, ov, path) {
                    return Some(d);
                }
                path.truncate(l2);
            }
            if ea.len() != oa.len() {
                let _ = write!(path, ".length");
                return Some(mk(path, Some(&Value::from(ea.len())), Some(&Value::from(oa.len()))));
            }
            None
        }
        (Value::Number(_), Value::Number(_)) if num_eq(e, o) => None,
        _ if e == o => None,
        _ => Some(mk(path, Some(e), Some(o))),
    }
}

// ---------------------------------------------------------------------------------------------
// outcomes, guarded execution

#[derive(Debug, Clone)]
pub enum Outcome {
    Pass,
    AstMismatch(Diff),
    ErrorMismatch(String),
    /// `todo!()` / `unimplemented!()` reached.
    Unimplemented(String),
    Panicked(String),
    /// Case not runnable (unsupported option, upstream crash, unrepresentable source).
    Skipped(String),
}

impl Outcome {
    pub fn category(&self) -> &'static str {
        match self {
            Outcome::Pass => "pass",
            Outcome::AstMismatch(_) => "ast mismatch",
            Outcome::ErrorMismatch(_) => "error mismatch",
            Outcome::Unimplemented(_) => "unimplemented",
            Outcome::Panicked(_) => "panicked",
            Outcome::Skipped(_) => "skipped",
        }
    }
    pub fn detail(&self) -> String {
        match self {
            Outcome::Pass => String::new(),
            Outcome::AstMismatch(d) => d.to_string(),
            Outcome::ErrorMismatch(s) | Outcome::Unimplemented(s) | Outcome::Panicked(s) | Outcome::Skipped(s) => s.clone(),
        }
    }
}

thread_local! {
    static QUIET: Cell<bool> = const { Cell::new(false) };
}
static HOOK: Once = Once::new();

/// Runs `f`, turning a panic into `Outcome::Unimplemented` (todo!/unimplemented!) or
/// `Outcome::Panicked`. The panic message is not printed (thousands of cases would flood the log).
pub fn guarded<T>(f: impl FnOnce() -> T) -> Result<T, Outcome> {
    HOOK.call_once(|| {
        let prev = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if !QUIET.with(Cell::get) {
                prev(info)
            }
        }));
    });
    QUIET.with(|q| q.set(true));
    let r = panic::catch_unwind(AssertUnwindSafe(f));
    QUIET.with(|q| q.set(false));
    r.map_err(|payload| {
        let msg = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic payload>".into());
        if msg.starts_with("not yet implemented") || msg.starts_with("not implemented") {
            Outcome::Unimplemented(msg)
        } else {
            Outcome::Panicked(msg)
        }
    })
}

/// Maps `f` over `items` on all cores (large stacks: recursive descent parsers on deep inputs),
/// preserving order.
pub fn parallel_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let n = std::thread::available_parallelism().map_or(4, |n| n.get()).min(16).max(1);
    let chunk = items.len().div_ceil(n).max(1);
    let f = &f;
    std::thread::scope(|s| {
        let handles: Vec<_> = items
            .chunks(chunk)
            .map(|c| {
                std::thread::Builder::new()
                    .stack_size(256 << 20)
                    .spawn_scoped(s, move || c.iter().map(f).collect::<Vec<R>>())
                    .expect("spawn oracle worker")
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("oracle worker panicked")).collect()
    })
}

/// Cases whose id contains `$ORACLE_FILTER` (all when unset).
pub fn filter_cases(cases: Vec<Value>) -> Vec<Value> {
    match std::env::var("ORACLE_FILTER") {
        Ok(f) if !f.is_empty() => cases.into_iter().filter(|c| c["id"].as_str().is_some_and(|id| id.contains(&f))).collect(),
        _ => cases,
    }
}

// ---------------------------------------------------------------------------------------------
// reporting and the baseline ratchet

pub struct Report {
    pub suite: Suite,
    pub results: Vec<(String, Outcome)>,
    pub millis: u128,
}

impl Report {
    pub fn count(&self, cat: &str) -> usize {
        self.results.iter().filter(|(_, o)| o.category() == cat).count()
    }
    pub fn passes(&self) -> usize {
        self.count("pass")
    }
}

/// Runs every case through `run` in parallel.
pub fn run_suite(suite: Suite, cases: &[Value], run: impl Fn(&Value) -> Outcome + Sync) -> Report {
    let t = Instant::now();
    let results = parallel_map(cases, |c| (c["id"].as_str().unwrap_or("?").to_string(), run(c)));
    Report { suite, results, millis: t.elapsed().as_millis() }
}

const CATEGORIES: &[&str] = &["pass", "ast mismatch", "error mismatch", "unimplemented", "panicked", "skipped"];

/// Prints the summary (+ first `$ORACLE_SHOW` (default 20) failures), writes
/// `<oracle_dir>/<suite>-report.txt` with every non-passing case, then applies the ratchet.
pub fn finish(report: &Report) {
    let name = report.suite.name();
    let show: usize = std::env::var("ORACLE_SHOW").ok().and_then(|s| s.parse().ok()).unwrap_or(20);
    let mut out = String::new();
    let total = report.results.len();
    let _ = writeln!(out, "oracle[{name}]: {total} cases in {} ms", report.millis);
    for cat in CATEGORIES {
        let n = report.count(cat);
        let _ = writeln!(out, "  {cat:<15} {n:>6}  ({:.1}%)", if total == 0 { 0.0 } else { n as f64 * 100.0 / total as f64 });
    }
    // histograms: skip reasons, first-diff key of AST mismatches, panic messages
    let mut hist: BTreeMap<String, usize> = BTreeMap::new();
    for (_, o) in &report.results {
        let key = match o {
            Outcome::Skipped(s) => format!("skipped: {s}"),
            Outcome::AstMismatch(d) => {
                format!("ast mismatch at key `{}`", d.path.rsplit(['.', '>']).next().unwrap_or(&d.path).split('[').next().unwrap_or(""))
            }
            Outcome::Unimplemented(s) | Outcome::Panicked(s) => format!("{}: {}", o.category(), s.lines().next().unwrap_or("")),
            Outcome::ErrorMismatch(s) => format!("error mismatch: {}", s.split(':').next().unwrap_or("")),
            Outcome::Pass => continue,
        };
        *hist.entry(key).or_default() += 1;
    }
    let mut hist: Vec<_> = hist.into_iter().collect();
    hist.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let _ = writeln!(out, "  top reasons:");
    for (k, n) in hist.iter().take(15) {
        let _ = writeln!(out, "    {n:>6}  {k}");
    }
    let failing: Vec<_> = report.results.iter().filter(|(_, o)| !matches!(o, Outcome::Pass | Outcome::Skipped(_))).collect();
    let mut details = String::new();
    for (id, o) in failing.iter().take(show) {
        let _ = writeln!(details, "  - {id} [{}]\n      {}", o.category(), o.detail());
    }
    if !details.is_empty() {
        let _ = writeln!(out, "  first {} of {} failing cases (ORACLE_SHOW=n for more, ORACLE_FILTER=id-substring to select):", show.min(failing.len()), failing.len());
        out.push_str(&details);
    }
    print!("{out}");

    let mut file = out.clone();
    let _ = writeln!(file, "\nall non-passing cases:");
    for (id, o) in &report.results {
        if !matches!(o, Outcome::Pass) {
            let _ = writeln!(file, "{id}\t{}\t{}", o.category(), o.detail().replace('\n', "\\n"));
        }
    }
    let path = oracle_dir().join(format!("{name}-report.txt"));
    match std::fs::write(&path, file) {
        Ok(()) => println!("  report: {}", path.display()),
        Err(e) => println!("  (could not write {}: {e})", path.display()),
    }
    ratchet(report.suite, report.passes());
}

pub fn read_baseline() -> Map<String, Value> {
    let text = std::fs::read_to_string(baseline_path()).expect("tests/oracle_baseline.json missing");
    serde_json::from_str::<Value>(&text).expect("oracle_baseline.json is not JSON").as_object().cloned().expect("baseline must be an object")
}

/// Fails if `passes` dropped below the checked-in baseline; with `ORACLE_UPDATE_BASELINE=1`
/// raises (never lowers) it. A filtered run (`ORACLE_FILTER`) neither checks nor updates.
pub fn ratchet(suite: Suite, passes: usize) {
    if std::env::var("ORACLE_FILTER").is_ok_and(|f| !f.is_empty()) {
        println!("  baseline: not checked (ORACLE_FILTER set)");
        return;
    }
    let name = suite.name();
    let mut base = read_baseline();
    let min = base.get(name).and_then(Value::as_u64).unwrap_or(0) as usize;
    if std::env::var("ORACLE_UPDATE_BASELINE").is_ok_and(|v| v == "1") && passes > min {
        base.insert(name.to_string(), Value::from(passes));
        let text = serde_json::to_string_pretty(&Value::Object(base)).unwrap() + "\n";
        std::fs::write(baseline_path(), text).expect("write oracle_baseline.json");
        println!("  baseline: raised {name} {min} -> {passes}");
        return;
    }
    assert!(
        passes >= min,
        "oracle[{name}] regression: {passes} passing cases < baseline {min} (tests/oracle_baseline.json)"
    );
    if passes > min {
        println!("  baseline: {passes} > {min}; run with ORACLE_UPDATE_BASELINE=1 to raise it");
    } else {
        println!("  baseline: {passes} (= {min})");
    }
}
