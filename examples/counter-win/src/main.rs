//! `counter-win`: a fixture bundle in the host on the WinUI 3 backend.
//!
//! ```text
//! counter-win [--fixture <name|dir>] [--selftest] [--report <json>] [--timeout <secs>]
//! counter-win --start-test <start-with|run-with|preattach> [--report <json>]
//! counter-win --na-fixture <title>
//! ```
//!
//! `--stall-ms <ms>` blocks the UI thread for that long before the loop starts (a slow first frame), so every
//! selftest timer is already due when the loop begins; the selftest must pass that way too (WG3w).
//!
//! Without `--selftest` it opens the window and runs until the user closes it. `--selftest`
//! (counter-win fixture; `--fixture winui-types` has its own checks, N4) drives the app through the real toolkit: clicks the button three
//! times through UI Automation, types a name, moves the slider, checks the box, then reads the
//! native widgets back and prints a JSON report (also written to `--report`). Exit code 0 = pass.
//! With `--fixture gallery-winui` (WP-W) it clicks every sidebar button of the gallery instead and checks that each
//! page created the kinds its `<!-- kinds: ... -->` header lists, with no JavaScript error.
//!
//! `--start-test` (G0, ABI v2.7 `start`, FU1) checks one host order on the counter-win bundle and
//! reports the same way: `start-with` mounts in `Host::start_with`'s hook while it pumps messages
//! (no `run`), then `stop` detaches; `run-with` mounts in `Host::run_with`'s hook from inside `run`;
//! `preattach` is the desktop order without `attach_host` (`load_bundle` attaches before mount).
//!
//! `--na-fixture <title>` (WG3w) opens the NA1 fixture of the WinUI capture (`tests/na_capture.rs`): a window
//! titled `<title>` holding two `TextBox`es and a `Button` (`Done`) in a vertical `StackPanel`, made through the
//! backend's ABI (no JavaScript, no bundle). It runs until its stdin closes; the capture drives it from outside
//! through `SendInput` and UI Automation.
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
        start_test: Option<String>,
        na_fixture: Option<String>,
        stall_ms: u64,
    }

    fn parse() -> Result<Args, String> {
        let mut a = Args { fixture: "counter-win".into(), selftest: false, report: None, timeout: 60.0, start_test: None, na_fixture: None, stall_ms: 0 };
        let mut it = std::env::args().skip(1);
        while let Some(x) = it.next() {
            match x.as_str() {
                "--fixture" => a.fixture = it.next().ok_or("--fixture needs a value")?,
                "--selftest" => a.selftest = true,
                "--report" => a.report = Some(it.next().ok_or("--report needs a value")?.into()),
                "--timeout" => a.timeout = it.next().ok_or("--timeout needs seconds")?.parse().map_err(|_| "bad --timeout")?,
                "--start-test" => a.start_test = Some(it.next().ok_or("--start-test needs a mode")?),
                "--na-fixture" => a.na_fixture = Some(it.next().ok_or("--na-fixture needs a title")?),
                "--stall-ms" => a.stall_ms = it.next().ok_or("--stall-ms needs milliseconds")?.parse().map_err(|_| "bad --stall-ms")?,
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

    /// The counter's "Count: n" text and its button, from the first window of `host`.
    fn counter_parts(b: &WinUiBackend, host: &Host<'_>) -> Result<(u64, u64), String> {
        let root = *host.toplevel_handles().first().ok_or("no window")?;
        let mut all = Vec::new();
        walk(b, root, &mut all);
        let find = |tag: &str| all.iter().find(|(_, t)| *t == tag).map(|(h, _)| *h).ok_or(format!("no <{tag}>"));
        Ok((find("text-block")?, find("button")?))
    }

    /// `AppBarButton` -> `app-bar-button` (the tags of the gallery's kind headers).
    fn kebab(name: &str) -> String {
        let mut out = String::new();
        for (i, c) in name.chars().enumerate() {
            if c.is_ascii_uppercase() && i > 0 {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        }
        out
    }

    /// The gallery pages (file stem, tags from the `<!-- kinds: ... -->` header), sorted like the sidebar.
    fn gallery_pages() -> Result<Vec<(String, Vec<String>)>, String> {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gallery-winui/src/pages");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "svelte"))
            .collect();
        files.sort();
        let mut out = Vec::new();
        for f in files {
            let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
            let kinds = text
                .split_once("<!-- kinds:")
                .and_then(|(_, rest)| rest.split_once("-->"))
                .map(|(k, _)| k.split_whitespace().map(kebab).collect())
                .unwrap_or_default();
            out.push((f.file_stem().unwrap().to_string_lossy().into_owned(), kinds));
        }
        Ok(out)
    }

    /// The gallery's sidebar buttons: the buttons of the first stack-panel under the window.
    fn sidebar(b: &WinUiBackend, host: &Host<'_>) -> Result<Vec<u64>, String> {
        let root = *host.toplevel_handles().first().ok_or("no window")?;
        let mut all = Vec::new();
        walk(b, root, &mut all);
        let panel = all.iter().find(|(_, t)| *t == "stack-panel").map(|(h, _)| *h).ok_or("no sidebar stack-panel")?;
        let tag = |h: u64| b.handle_info(h).ok().and_then(|i| svelters_winui::table::kind(i.kind)).map_or("?", |k| k.tag);
        Ok(b.children(panel, 0).into_iter().filter(|h| tag(*h) == "button").collect())
    }

    /// One line per page of `fixtures/gallery-winui` (fixtures/gallery-common/check-pages.mjs enforces the line; the
    /// selftest fails when this list and the pages directory differ). The kinds a page must create come from its
    /// `<!-- kinds: ... -->` header.
    macro_rules! gallery_page {
        ($name:ident, "winui", $page:literal) => {
            $page
        };
    }
    const GALLERY_CASES: &[&str] = &[
        gallery_page!(layout, "winui", "00-layout"),
        gallery_page!(buttons, "winui", "01-buttons"),
        gallery_page!(input, "winui", "02-input"),
        gallery_page!(pickers, "winui", "03-pickers"),
        gallery_page!(display, "winui", "04-display"),
        gallery_page!(lists, "winui", "05-lists"),
        gallery_page!(navigation, "winui", "06-navigation"),
        gallery_page!(menus, "winui", "07-menus"),
        gallery_page!(dialogs, "winui", "08-dialogs"),
    ];

    /// The gallery selftest's state, shared by its timer steps.
    struct Gallery {
        b: WinUiBackend,
        host: Rc<Host<'static>>,
        pages: Vec<(String, Vec<String>)>,
        seen: RefCell<Vec<Vec<String>>>,
        errors: RefCell<Vec<String>>,
        report: Rc<RefCell<Json>>,
        path: Option<PathBuf>,
    }

    /// The tags under the window, depth first.
    fn gallery_tags(g: &Gallery) -> Vec<String> {
        let mut all = Vec::new();
        if let Some(root) = g.host.toplevel_handles().first() {
            walk(&g.b, *root, &mut all);
        }
        all.iter().map(|(_, t)| t.to_string()).collect()
    }

    /// Click sidebar button `i`, then poll for its page.
    fn gallery_click(g: Rc<Gallery>, i: usize) {
        let r = sidebar(&g.b, &g.host).and_then(|s| s.get(i).copied().ok_or(format!("no sidebar button #{i}"))).and_then(|btn| g.b.invoke(btn));
        if let Err(e) = r {
            g.errors.borrow_mut().push(format!("page {i}: click: {e}"));
        }
        let g2 = g.clone();
        if let Err(e) = g.b.after(200, move || gallery_poll(g2, i, 0)) {
            g.errors.borrow_mut().push(format!("page {i}: timer: {e}"));
        }
    }

    /// Every 200 ms until page `i` shows all its header kinds (at most 8 s: a slow page is still checked), then the
    /// next page or the report.
    fn gallery_poll(g: Rc<Gallery>, i: usize, n: u32) {
        let tags = gallery_tags(&g);
        let done = g.pages[i].1.iter().all(|k| tags.contains(k));
        if !done && n < 40 {
            let g2 = g.clone();
            if g.b.after(200, move || gallery_poll(g2, i, n + 1)).is_ok() {
                return;
            }
        }
        g.seen.borrow_mut()[i] = tags;
        if i + 1 < g.pages.len() {
            gallery_click(g, i + 1);
        } else {
            gallery_finish(&g);
        }
    }

    fn gallery_finish(g: &Gallery) {
        let mut failures: Vec<String> = g.errors.borrow().clone();
        let names: Vec<&str> = g.pages.iter().map(|(n, _)| n.as_str()).collect();
        if names != GALLERY_CASES {
            failures.push(format!("the pages {names:?} differ from GALLERY_CASES {GALLERY_CASES:?} (add a gallery_page! line)"));
        }
        let seen = g.seen.borrow();
        let mut shown = serde_json::Map::new();
        for (i, (name, kinds)) in g.pages.iter().enumerate() {
            let missing: Vec<&String> = kinds.iter().filter(|k| !seen[i].contains(k)).collect();
            if !missing.is_empty() {
                failures.push(format!("{name}: not created: {missing:?}"));
            }
            shown.insert(name.clone(), json!({ "kinds": kinds.len(), "missing": missing, "handles": seen[i].len() }));
        }
        if sidebar(&g.b, &g.host).map(|s| s.len()).unwrap_or(0) != g.pages.len() {
            failures.push(format!("the sidebar does not have {} buttons", g.pages.len()));
        }
        failures.extend(g.host.take_errors().iter().map(|e| format!("js error: {e}")));
        let mut rep = g.report.borrow_mut();
        *rep = json!({
            "backend": "winui",
            "fixture": "gallery-winui",
            "ok": failures.is_empty(),
            "failures": failures,
            "pages": shown,
            "console": g.host.take_console().iter().map(|l| format!("[{}] {}", l.level, l.text)).collect::<Vec<_>>(),
        });
        if let Some(p) = &g.path {
            let _ = std::fs::write(p, serde_json::to_string_pretty(&*rep).unwrap());
        }
        g.b.quit();
    }

    /// `--fixture gallery-winui --selftest`: from 800 ms on, click each sidebar button in turn; after each click, wait
    /// until the page's `<!-- kinds -->` exist under the window, then the next. The report lists what each page created.
    fn gallery_selftest(b: &WinUiBackend, host: &Rc<Host<'static>>, report: Rc<RefCell<Json>>, path: Option<PathBuf>) -> Result<(), String> {
        let pages = gallery_pages()?;
        if pages.is_empty() {
            return Err("no gallery pages".into());
        }
        let n = pages.len();
        let g = Rc::new(Gallery {
            b: b.clone(),
            host: host.clone(),
            pages,
            seen: RefCell::new(vec![Vec::new(); n]),
            errors: RefCell::new(Vec::new()),
            report,
            path,
        });
        b.after(800, move || gallery_click(g, 0))
    }

    /// `--start-test <mode>` (G0): one host order on the counter-win bundle; a JSON report like `--selftest`.
    fn start_test(mode: &str, b: &WinUiBackend, vt: &'static BackendVtable, args: &Args) -> i32 {
        let path = bundle(&args.fixture);
        let mut failures: Vec<String> = Vec::new();
        let console: Vec<String>;
        {
            let host = match Host::new(vt.backend_ref(), HostConfig::default()) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("[counter-win] host: {e}");
                    return 3;
                }
            };
            let br = vt.backend_ref();
            let mut check = |ok: bool, what: &str| {
                if !ok {
                    failures.push(what.to_string());
                }
            };
            let count = |b: &WinUiBackend, host: &Host<'_>| -> String {
                match counter_parts(b, host).and_then(|(t, _)| prop(b, t, "text")) {
                    Ok(Value::Str(s)) => s,
                    other => format!("{other:?}"),
                }
            };
            let click = |b: &WinUiBackend, host: &Host<'_>| -> Result<(), String> {
                let (_, button) = counter_parts(b, host)?;
                b.invoke(button)?;
                b.pump(300);
                Ok(())
            };
            check(br.has_start(), "the vtable publishes start");
            match mode {
                "start-with" => {
                    let fired = Rc::new(std::cell::Cell::new(0));
                    let (f, p) = (fired.clone(), path.clone());
                    let r = host.start_with(move |h| {
                        f.set(f.get() + 1);
                        if let Err(e) = h.load_bundle(&p) {
                            eprintln!("[counter-win] load: {e}");
                        }
                    });
                    check(r.is_ok(), &format!("start_with: {r:?}"));
                    check(fired.get() == 1, "the hook ran when start returned");
                    check(host.is_started(), "is_started after start");
                    let before = host.checkpoint_count();
                    b.pump(300);
                    check(host.checkpoint_count() > before, "start queued a checkpoint for the first pump");
                    let c = count(b, &host);
                    check(c == "Count: 0", &format!("mounted: {c}"));
                    let r = click(b, &host);
                    check(r.is_ok(), &format!("click: {r:?}"));
                    let c = count(b, &host);
                    check(c == "Count: 1", &format!("click reached the started host: {c}"));
                    host.stop();
                    check(!host.is_started(), "not started after stop");
                    let before = host.checkpoint_count();
                    br.wake();
                    b.pump(200);
                    check(host.checkpoint_count() == before, "no checkpoint after stop");
                    let _ = click(b, &host);
                    let c = count(b, &host);
                    check(c == "Count: 1", &format!("no event after stop: {c}"));
                }
                "run-with" => {
                    let fired = Rc::new(std::cell::Cell::new(0));
                    let (f, p, d) = (fired.clone(), path.clone(), b.clone());
                    let r = host.run_with(move |h| {
                        f.set(f.get() + 1);
                        if !h.is_started() {
                            eprintln!("[counter-win] the hook ran before started");
                        }
                        if let Err(e) = h.load_bundle(&p) {
                            eprintln!("[counter-win] load: {e}");
                        }
                        d.quit();
                    });
                    check(r.is_ok(), &format!("run_with: {r:?}"));
                    check(fired.get() == 1, &format!("the hook fired once (got {})", fired.get()));
                    check(!host.is_started(), "not started after run returned");
                    let c = count(b, &host);
                    check(c == "Count: 0", &format!("mounted: {c}"));
                }
                "preattach" => {
                    let r = host.load_bundle(&path);
                    check(r.is_ok(), &format!("load_bundle: {:?}", r.err().map(|e| e.to_string())));
                    check(!host.is_started(), "the pre-attach does not count as started");
                    let before = host.checkpoint_count();
                    br.wake();
                    b.pump(200);
                    check(host.checkpoint_count() > before, "a wake before run reaches the host");
                    let r = click(b, &host);
                    check(r.is_ok(), &format!("click: {r:?}"));
                    let c = count(b, &host);
                    check(c == "Count: 1", &format!("click reached the pre-attached host: {c}"));
                }
                other => {
                    eprintln!("[counter-win] unknown --start-test mode {other:?}");
                    return 2;
                }
            }
            failures.extend(host.take_errors().iter().map(|e| format!("js error: {e}")));
            console = host.take_console().iter().map(|l| format!("[{}] {}", l.level, l.text)).collect();
            // Drop detaches the host (start(NULL)): later wakes and timers reach nobody.
        }
        vt.backend_ref().wake();
        b.pump(50);
        b.shutdown();
        let rep = json!({ "backend": "winui", "start_test": mode, "ok": failures.is_empty(), "failures": failures, "console": console });
        if let Some(p) = &args.report {
            let _ = std::fs::write(p, serde_json::to_string_pretty(&rep).unwrap());
        }
        println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        if rep["ok"] == json!(true) { 0 } else { 1 }
    }

    /// `--na-fixture <title>` (WG3w): the NA1 fixture window, until stdin closes.
    fn na_fixture(b: &WinUiBackend, title: &str) -> i32 {
        let build = || -> Result<(), String> {
            let make = |tag: &str| -> Result<u64, String> {
                let k = b.resolve(NameKind::Kind, 0, tag);
                if k == 0 {
                    return Err(format!("no kind <{tag}>"));
                }
                b.create(k)
            };
            let win = make("window")?;
            set(b, win, "title", Value::Str(title.into()))?;
            let panel = make("stack-panel")?;
            set(b, panel, "orientation", Value::Str("vertical".into()))?;
            set(b, panel, "spacing", Value::Str("8".into()))?;
            set(b, panel, "padding", Value::Str("16".into()))?;
            for _ in 0..2 {
                let tb = make("text-box")?;
                b.insert(panel, tb, None, 0)?;
            }
            let button = make("button")?;
            set(b, button, "content", Value::Str("Done".into()))?;
            b.insert(panel, button, None, 0)?;
            b.insert(win, panel, None, 0)?;
            let kind = b.handle_info(win)?.kind;
            b.call(win, b.resolve(NameKind::Method, kind, "activate"), &[])?;
            Ok(())
        };
        if let Err(e) = build() {
            eprintln!("[counter-win] na-fixture: {e}");
            return 3;
        }
        let open = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let flag = open.clone();
        std::thread::spawn(move || {
            let mut sink = Vec::new();
            let _ = std::io::Read::read_to_end(&mut std::io::stdin(), &mut sink);
            flag.store(false, std::sync::atomic::Ordering::SeqCst);
        });
        while open.load(std::sync::atomic::Ordering::SeqCst) {
            b.pump(20);
        }
        b.shutdown();
        0
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
        if let Some(mode) = &args.start_test {
            return start_test(mode, &driver, vt, &args);
        }
        if let Some(title) = &args.na_fixture {
            let _ = vt;
            return na_fixture(&driver, title);
        }
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
        let types = args.fixture == "winui-types";
        if args.selftest && args.fixture == "gallery-winui" {
            let b3 = driver.clone();
            let ok = gallery_selftest(&driver, &host, report.clone(), args.report.clone())
                .and_then(|_| driver.after((args.timeout * 1000.0) as u64, move || b3.quit()));
            if let Err(e) = ok {
                eprintln!("[counter-win] scheduling the gallery selftest: {e}");
                return 3;
            }
        } else if args.selftest {
            let (b, h, r) = (driver.clone(), host.clone(), report.clone());
            let steps = move || -> Result<(), String> {
                let root = *h.toplevel_handles().first().ok_or("no window")?;
                let mut all = Vec::new();
                walk(&b, root, &mut all);
                let find = |tag: &str, n: usize| all.iter().filter(|(_, t)| *t == tag).nth(n).map(|(h, _)| *h).ok_or(format!("no <{tag}> #{n}"));
                let button = find("button", 0)?;
                if types {
                    // fixtures/winui-types: one click inserts into the PropertySet and recolors the brush.
                    b.invoke(button)?;
                    let tree: Vec<Json> = all.iter().map(|(h, t)| json!([h, t])).collect();
                    *r.borrow_mut() = json!({ "tree": tree });
                    return Ok(());
                }
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
                if let Some(root) = h2.toplevel_handles().first() {
                    walk(&b2, *root, &mut all);
                }
                let find = |tag: &str, n: usize| all.iter().filter(|(_, t)| *t == tag).nth(n).map(|(h, _)| *h);
                let read = |tag: &str, n: usize, p: &str| find(tag, n).map(|h| text(prop(&b2, h, p))).unwrap_or(Json::Null);
                let (got, want) = if types {
                    // fixtures/winui-types (N4): non-GUI WinRT types used from Svelte.
                    let got = json!({
                        "uri": read("text-block", 0, "text"),
                        "map": read("text-block", 1, "text"),
                        "brush": read("text-block", 2, "text"),
                        "async": read("text-block", 3, "text"),
                    });
                    let want = json!({
                        "uri": "example.com:8080/docs",
                        "map": "size 3, changes 3",
                        "brush": "brush 0,0,255 @ 0.5",
                        "async": "pasted copied",
                    });
                    (got, want)
                } else {
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
                    (got, want)
                };
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
        if args.stall_ms > 0 {
            // A UI thread busy before its loop starts, as a slow first frame on CI's software-rendered runners
            // (WG3w: the selftest's timers are all due when the loop starts).
            std::thread::sleep(std::time::Duration::from_millis(args.stall_ms));
        }
        let ran = host.run();
        for e in host.take_errors() {
            eprintln!("[counter-win] {e}");
        }
        // Close the windows and shut XAML down on this thread before the process exits (the
        // backend in the leaked vtable never drops; exiting with live XAML fail-fasts).
        drop(host);
        driver.shutdown();
        if let Err(e) = ran {
            eprintln!("[counter-win] run: {e}");
            return 5;
        }
        if args.selftest {
            let rep = report.borrow();
            println!("{}", serde_json::to_string_pretty(&*rep).unwrap());
            return if rep["ok"] == json!(true) { 0 } else { 1 };
        }
        0
    }
}
