//! `counter-win`: a fixture bundle in the host on the WinUI 3 backend.
//!
//! ```text
//! counter-win [--fixture <name|dir>] [--selftest] [--report <json>] [--timeout <secs>]
//! ```
//!
//! Without `--selftest` it opens the window and runs until the user closes it. `--selftest`
//! (counter-win fixture only) drives the app through the real toolkit: clicks the button three
//! times through UI Automation, types a name, moves the slider, checks the box, then reads the
//! native widgets back and prints a JSON report (also written to `--report`). Exit code 0 = pass.
//!
//! Build the bundle first: `pnpm install && pnpm --filter ./fixtures/counter-win build`.
//! Setup on Windows: docs/notes/windows-setup.md.

#[cfg(not(windows))]
fn main() {
    eprintln!("counter-win runs on Windows only (WinUI 3). See docs/notes/windows-setup.md.");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() {
    std::process::exit(win::main());
}

#[cfg(windows)]
mod win {
    use serde_json::{Value as Json, json};
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::time::Instant;
    use svelters_abi::{Backend, BackendVtable, NameKind, Value};
    use svelters_host::{Host, HostConfig};
    use svelters_winui::WinUiBackend;

    struct Args {
        fixture: String,
        selftest: bool,
        report: Option<PathBuf>,
        timeout: f64,
    }

    fn parse() -> Result<Args, String> {
        let mut a = Args { fixture: "counter-win".into(), selftest: false, report: None, timeout: 60.0 };
        let mut it = std::env::args().skip(1);
        while let Some(x) = it.next() {
            match x.as_str() {
                "--fixture" => a.fixture = it.next().ok_or("--fixture needs a value")?,
                "--selftest" => a.selftest = true,
                "--report" => a.report = Some(it.next().ok_or("--report needs a value")?.into()),
                "--timeout" => a.timeout = it.next().ok_or("--timeout needs seconds")?.parse().map_err(|_| "bad --timeout")?,
                "-h" | "--help" => return Err("usage: counter-win [--fixture <name|dir>] [--selftest] [--report <json>] [--timeout <secs>]".into()),
                other => return Err(format!("unknown argument {other:?}")),
            }
        }
        Ok(a)
    }

    fn bundle(fixture: &str) -> PathBuf {
        let dir = if fixture.contains('/') || fixture.contains('\\') {
            PathBuf::from(fixture)
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(fixture)
        };
        dir.join("dist").join("app.js")
    }

    /// Every handle below `h` (depth first, `h` included) with its tag.
    fn walk(b: &WinUiBackend, h: u64, out: &mut Vec<(u64, &'static str)>) {
        let tag = b.handle_info(h).ok().and_then(|i| svelters_winui::table::kind(i.kind)).map_or("?", |k| k.tag);
        out.push((h, tag));
        for c in b.children(h, 0) {
            walk(b, c, out);
        }
    }

    fn prop(b: &WinUiBackend, h: u64, name: &str) -> Result<Value, String> {
        let kind = b.handle_info(h)?.kind;
        b.get_prop(h, b.resolve(NameKind::Prop, kind, name))
    }

    fn set(b: &WinUiBackend, h: u64, name: &str, v: Value) -> Result<(), String> {
        let kind = b.handle_info(h)?.kind;
        b.set_prop(h, b.resolve(NameKind::Prop, kind, name), &v)
    }

    fn text(v: Result<Value, String>) -> Json {
        match v {
            Ok(Value::Str(s)) => json!(s),
            Ok(other) => json!(format!("{other:?}")),
            Err(e) => json!({ "error": e }),
        }
    }

    pub fn main() -> i32 {
        let args = match parse() {
            Ok(a) => a,
            Err(e) => {
                eprintln!("{e}");
                return 2;
            }
        };
        let start = Instant::now();
        let backend = match WinUiBackend::new() {
            Ok(b) => b,
            Err(e) => {
                eprintln!("[counter-win] WinUI init failed: {e}");
                if let Some(p) = &args.report {
                    let rep = json!({ "backend": "winui", "ok": false, "failures": [format!("WinUI init: {e}")] });
                    let _ = std::fs::write(p, serde_json::to_string_pretty(&rep).unwrap());
                }
                return 3;
            }
        };
        let driver = backend.clone();
        let vt: &'static BackendVtable = Box::leak(Box::new(BackendVtable::new(backend)));
        let host = match Host::new(vt.backend_ref(), HostConfig::default()) {
            Ok(h) => Rc::new(h),
            Err(e) => {
                eprintln!("[counter-win] host: {e}");
                return 3;
            }
        };
        let path = bundle(&args.fixture);
        if !path.is_file() {
            eprintln!("[counter-win] {} not found: pnpm install && pnpm --filter ./fixtures/{} build", path.display(), args.fixture);
            return 3;
        }
        if let Err(e) = host.load_bundle(&path) {
            eprintln!("[counter-win] load {}: {e}", path.display());
            return 4;
        }
        let load_ms = start.elapsed().as_secs_f64() * 1e3;
        let report: Rc<RefCell<Json>> = Rc::new(RefCell::new(json!({ "ok": false, "failures": ["selftest did not finish"] })));
        if args.selftest {
            let (b, h, r) = (driver.clone(), host.clone(), report.clone());
            let steps = move || -> Result<(), String> {
                let root = *h.toplevels().first().ok_or("no window")?;
                let mut all = Vec::new();
                walk(&b, root, &mut all);
                let find = |tag: &str, n: usize| all.iter().filter(|(_, t)| *t == tag).nth(n).map(|(h, _)| *h).ok_or(format!("no <{tag}> #{n}"));
                let button = find("button", 0)?;
                for _ in 0..3 {
                    b.invoke(button)?;
                }
                set(&b, find("text-box", 0)?, "text", Value::Str("Ada".into()))?;
                set(&b, find("slider", 0)?, "value", Value::F64(42.0))?;
                set(&b, find("check-box", 0)?, "is-checked", Value::Bool(true))?;
                let tree: Vec<Json> = all.iter().map(|(h, t)| json!([h, t])).collect();
                *r.borrow_mut() = json!({ "tree": tree });
                Ok(())
            };
            let (b2, h2, r2, report_path) = (driver.clone(), host.clone(), report.clone(), args.report.clone());
            let check = move || {
                let mut failures: Vec<String> = Vec::new();
                let mut all = Vec::new();
                if let Some(root) = h2.toplevels().first() {
                    walk(&b2, *root, &mut all);
                }
                let find = |tag: &str, n: usize| all.iter().filter(|(_, t)| *t == tag).nth(n).map(|(h, _)| *h);
                let read = |tag: &str, n: usize, p: &str| find(tag, n).map(|h| text(prop(&b2, h, p))).unwrap_or(Json::Null);
                let got = json!({
                    "count": read("text-block", 0, "text"),
                    "hello": read("text-block", 1, "text"),
                    "button": read("button", 0, "content"),
                    "check": read("check-box", 0, "content"),
                    "title": read("window", 0, "title"),
                });
                let want = json!({
                    "count": "Count: 3",
                    "hello": "Hello Ada",
                    "button": "Increment",
                    "check": "Level 42, agree: yes",
                    "title": "Counter",
                });
                for (k, v) in want.as_object().unwrap() {
                    if got[k] != *v {
                        failures.push(format!("{k}: expected {v}, got {}", got[k]));
                    }
                }
                let errors: Vec<String> = h2.take_errors().iter().map(|e| e.to_string()).collect();
                failures.extend(errors.iter().map(|e| format!("js error: {e}")));
                let mut rep = r2.borrow_mut();
                let tree = rep.get("tree").cloned().unwrap_or(Json::Null);
                *rep = json!({
                    "backend": "winui",
                    "ok": failures.is_empty(),
                    "failures": failures,
                    "read_back": got,
                    "tree": tree,
                    "console": h2.take_console().iter().map(|l| format!("[{}] {}", l.level, l.text)).collect::<Vec<_>>(),
                    "load_ms": load_ms,
                });
                if let Some(p) = &report_path {
                    let _ = std::fs::write(p, serde_json::to_string_pretty(&*rep).unwrap());
                }
                b2.quit();
            };
            let r3 = report.clone();
            let ok = driver.after(800, move || {
                if let Err(e) = steps() {
                    *r3.borrow_mut() = json!({ "ok": false, "failures": [format!("driving: {e}")] });
                }
            });
            let ok = ok.and_then(|_| driver.after(2500, check));
            let b3 = driver.clone();
            let ok = ok.and_then(|_| driver.after((args.timeout * 1000.0) as u64, move || b3.quit()));
            if let Err(e) = ok {
                eprintln!("[counter-win] scheduling the selftest: {e}");
                return 3;
            }
        }
        if let Err(e) = host.run() {
            eprintln!("[counter-win] run: {e}");
            return 5;
        }
        if args.selftest {
            let rep = report.borrow();
            println!("{}", serde_json::to_string_pretty(&*rep).unwrap());
            return if rep["ok"] == json!(true) { 0 } else { 1 };
        }
        for e in host.take_errors() {
            eprintln!("[counter-win] {e}");
        }
        0
    }
}
