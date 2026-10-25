//! AU1b: one test body, in process or out of process (E2E-INSPECTOR §2.3).
//!
//! ```ignore
//! use svelters_automation::e2e::prelude::*;
//!
//! #[svelters_automation::test(app = "../../fixtures/counter-egui", seed = 3, iterations = 4)]
//! fn increments(d: &mut E2e) -> TestResult {
//!     d.get_by_role("button", name("Increment")).click()?;
//!     expect(d.get_by_text_exact("Count: 1")).to_be_visible()?;
//!     d.clock().run_for(250)?;
//!     Ok(())
//! }
//! ```
//!
//! The attribute ([`crate::test`]) turns the function into a `#[test]` that calls [`run`]:
//!
//! - **In process** (the default): the app is compiled live from source on a kittest
//!   [`TestApp`] (no window), under a [`Session`] with every grant, on a [`VirtualClock`] seeded with
//!   the test's seed (or the wall clock with `clock = "real"`).
//! - **Out of process** (`SVELTERS_E2E=process`): the reference host `svelters-run` (or the app
//!   binary in `SVELTERS_APP`, which takes the same flags) is spawned with `--headless --clock
//!   <clock> --seed <seed> --step-ms <step_ms>` and driven over AU2's transport ([`client::App`]).
//!   `svelters-run` is found through `SVELTERS_RUN`, `CARGO_BIN_EXE_svelters-run` (the tests of
//!   this crate), or next to the test binary (`target/<profile>/svelters-run`).
//!
//! Both paths go through the same JSON methods ([`Session::call`] and [`client::Client::call`] have
//! one signature), so the body cannot tell them apart. `iterations` reruns the body with seeds
//! `seed..seed + iterations`; `SEED=s` and `ITERATIONS=n` override them, and a failure prints the
//! seed and the command that replays it.
//!
//! The name is `E2e` rather than the spec's `Driver`: [`crate::Driver`] is the frame-loop trait.

use crate::client;
use crate::clock::{ClockMode, Clocked, RealTime, VirtualClock};
use crate::server::Endpoint;
use crate::{Driver, Error, Session};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::Duration;
use svelters_egui::harness::TestApp;
use svelters_host::{HostConfig, HostSched};

/// What a test body returns.
pub type TestResult = Result<(), Error>;

/// The imports of a test body.
pub mod prelude {
    pub use super::{ByRole, E2e, Expectation, Locator, TestResult, expect, name};
    pub use crate::Error;
    pub use serde_json::json;
}

/// The attribute's arguments; [`crate::test`] fills them in.
#[derive(Clone, Debug)]
pub struct TestSpec {
    pub name: &'static str,
    /// The app (a directory with `App.svelte`, the `.svelte` file, or a built `.js` bundle),
    /// relative to `manifest_dir`.
    pub app: &'static str,
    /// The test crate's `CARGO_MANIFEST_DIR` (apps and `tests/snapshots/` resolve against it).
    pub manifest_dir: &'static str,
    pub seed: u64,
    pub iterations: u64,
    /// `"virtual"` or `"real"`.
    pub clock: &'static str,
    pub step_ms: f64,
    /// `CARGO_BIN_EXE_svelters-run` where the test crate has it.
    pub run_bin: Option<&'static str>,
}

impl TestSpec {
    pub fn new(name: &'static str, app: &'static str, manifest_dir: &'static str) -> TestSpec {
        TestSpec {
            name,
            app,
            manifest_dir,
            seed: 0,
            iterations: 1,
            clock: "virtual",
            step_ms: 16.0,
            run_bin: None,
        }
    }
}

enum Inner {
    InProcess(Box<Session<Box<dyn Driver>>>),
    Process(client::App),
}

/// A running app under test, in process or out of process (see the module docs). Its methods
/// take `&self`, so locators can be held side by side.
pub struct E2e {
    inner: RefCell<Inner>,
    seed: u64,
    snapshots: PathBuf,
}

fn host_config(seed: Option<u64>) -> HostConfig {
    let sched = match seed {
        Some(s) => HostSched::default().seed(s),
        None => HostSched::default(),
    };
    HostConfig {
        echo_console: false,
        echo_errors: false,
        sched,
        ..HostConfig::default()
    }
}

/// An app compiled live from source, or a built bundle (`.js`, e.g. `dist/app.js`: apps that use
/// Vite-only features such as `import.meta.glob`).
fn test_app(app: &Path, dt: f32, cfg: HostConfig) -> Result<TestApp, Error> {
    if app.extension().is_some_and(|e| e == "js") {
        TestApp::load_with(app, dt, cfg).map_err(setup)
    } else {
        TestApp::from_source_with(app, dt, cfg).map_err(setup)
    }
}

fn setup(m: impl std::fmt::Display) -> Error {
    Error::new("test.setup", m.to_string())
}

impl E2e {
    /// The app in process, on a kittest [`TestApp`] compiled from source.
    pub fn in_process(app: &Path, clock: ClockMode, seed: u64, step_ms: f64) -> Result<E2e, Error> {
        let dt = (step_ms / 1000.0) as f32;
        let driver: Box<dyn Driver> = match clock {
            ClockMode::Virtual => {
                let c = VirtualClock::new(seed);
                let a = test_app(app, dt, c.config(host_config(None)))?;
                Box::new(Clocked::new(a, c))
            }
            ClockMode::Real => {
                let a = test_app(app, dt, host_config(Some(seed)))?;
                Box::new(RealTime::new(a))
            }
        };
        let mut s = Session::for_test(driver);
        s.wait_settled(10_000.0)?;
        Ok(E2e::new(Inner::InProcess(Box::new(s)), seed))
    }

    /// The app out of process: `bin [app] --headless --clock <clock> --seed <seed> --step-ms <ms>`.
    pub fn process(bin: &Path, app: Option<&Path>, clock: ClockMode, seed: u64, step_ms: f64) -> Result<E2e, Error> {
        let (seed_s, step_s) = (seed.to_string(), step_ms.to_string());
        let app_s = app.map(|a| a.to_string_lossy().into_owned());
        let mut args: Vec<&str> = app_s.iter().map(String::as_str).collect();
        args.extend(["--headless", "--clock", clock.as_str(), "--seed", &seed_s, "--step-ms", &step_s]);
        let a = client::App::spawn(bin, &args, &Endpoint::Tcp(0), Duration::from_secs(120))?;
        let d = E2e::new(Inner::Process(a), seed);
        d.call("session.hello", json!({"client": "svelters_automation::e2e"}))?;
        d.call("wait.settled", json!({"timeout": 10_000}))?;
        Ok(d)
    }

    fn new(inner: Inner, seed: u64) -> E2e {
        E2e { inner: RefCell::new(inner), seed, snapshots: PathBuf::from("tests/snapshots") }
    }

    /// Where [`Expectation::to_match_scene_snapshot`] keeps its files (default `tests/snapshots`).
    pub fn set_snapshot_dir(&mut self, dir: impl Into<PathBuf>) {
        self.snapshots = dir.into();
    }

    /// `"in-process"` or `"process"`.
    pub fn mode(&self) -> &'static str {
        match &*self.inner.borrow() {
            Inner::InProcess(_) => "in-process",
            Inner::Process(_) => "process",
        }
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Any protocol method, raw (§1.1).
    pub fn call(&self, method: &str, params: Value) -> Result<Value, Error> {
        match &mut *self.inner.borrow_mut() {
            Inner::InProcess(s) => s.call(method, &params),
            Inner::Process(a) => a.call(method, &params),
        }
    }

    /// The whole app (no scope).
    pub fn locator(&self, selector: &str) -> Locator<'_> {
        Locator { d: self, selector: selector.to_string() }
    }

    /// The root with this key (`root=key`).
    pub fn root(&self, key: &str) -> Locator<'_> {
        self.locator(&format!("root={key}"))
    }

    pub fn get_by_role(&self, role: &str, by: impl Into<ByRole>) -> Locator<'_> {
        Locator { d: self, selector: String::new() }.get_by_role(role, by)
    }

    pub fn get_by_text(&self, text: &str) -> Locator<'_> {
        Locator { d: self, selector: String::new() }.get_by_text(text)
    }

    pub fn get_by_text_exact(&self, text: &str) -> Locator<'_> {
        Locator { d: self, selector: String::new() }.get_by_text_exact(text)
    }

    pub fn get_by_label(&self, label: &str) -> Locator<'_> {
        Locator { d: self, selector: String::new() }.get_by_label(label)
    }

    pub fn press(&self, key: &str) -> Result<Value, Error> {
        self.call("input.press", json!({"key": key}))
    }

    pub fn type_text(&self, text: &str) -> Result<Value, Error> {
        self.call("input.type", json!({"text": text}))
    }

    /// Steps frames until settled; returns the frames run.
    pub fn wait_settled(&self) -> Result<u64, Error> {
        Ok(self.call("wait.settled", json!({}))?["frames"].as_u64().unwrap_or(0))
    }

    /// The whole scene's snapshot (aria-snapshot YAML).
    pub fn snapshot(&self) -> Result<String, Error> {
        Ok(self.call("scene.snapshot", json!({}))?["text"].as_str().unwrap_or_default().to_string())
    }

    /// The virtual clock (`clock.*`).
    pub fn clock(&self) -> ClockRef<'_> {
        ClockRef(self)
    }
}

/// `clock.*` of an [`E2e`].
pub struct ClockRef<'a>(&'a E2e);

impl ClockRef<'_> {
    /// Lets `ms` of virtual time pass, frame by frame; returns the frames run.
    pub fn run_for(&self, ms: f64) -> Result<u64, Error> {
        Ok(self.0.call("clock.advance", json!({"ms": ms}))?["frames"].as_u64().unwrap_or(0))
    }

    /// The virtual clock's milliseconds (`None`: the wall clock).
    pub fn now_ms(&self) -> Result<Option<f64>, Error> {
        Ok(self.0.call("clock.now", json!({}))?["ms"].as_f64())
    }
}

/// `get_by_role`'s accessible-name filter: `()` for none, [`name`] for an exact name.
#[derive(Clone, Debug, Default)]
pub struct ByRole {
    pub name: Option<String>,
    pub exact: bool,
}

/// An exact accessible name for `get_by_role`.
pub fn name(n: &str) -> ByRole {
    ByRole { name: Some(n.to_string()), exact: true }
}

impl ByRole {
    /// Matches names that contain this one.
    pub fn contains(mut self) -> ByRole {
        self.exact = false;
        self
    }
}

impl From<()> for ByRole {
    fn from(_: ()) -> ByRole {
        ByRole::default()
    }
}

impl From<&str> for ByRole {
    fn from(n: &str) -> ByRole {
        name(n)
    }
}

fn quote(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

/// A lazy selector, resolved in the app on every call (strictly, one node, for acts). The same
/// shapes as `@svelters/test`'s `Locator`.
#[derive(Clone)]
pub struct Locator<'a> {
    d: &'a E2e,
    selector: String,
}

impl<'a> Locator<'a> {
    pub fn selector(&self) -> &str {
        &self.selector
    }

    /// Narrows to `selector` inside this one (`a >> b`).
    pub fn locator(&self, selector: &str) -> Locator<'a> {
        let selector = if self.selector.is_empty() { selector.to_string() } else { format!("{} >> {selector}", self.selector) };
        Locator { d: self.d, selector }
    }

    pub fn get_by_role(&self, role: &str, by: impl Into<ByRole>) -> Locator<'a> {
        let by = by.into();
        match by.name {
            None => self.locator(&format!("role={role}")),
            Some(n) => self.locator(&format!("role={role}[name{}={}]", if by.exact { "" } else { "*" }, quote(&n))),
        }
    }

    /// Text that contains `text`.
    pub fn get_by_text(&self, text: &str) -> Locator<'a> {
        self.locator(&format!("text={text}"))
    }

    pub fn get_by_text_exact(&self, text: &str) -> Locator<'a> {
        self.locator(&format!("text={}", quote(text)))
    }

    pub fn get_by_label(&self, label: &str) -> Locator<'a> {
        self.locator(&format!("label={}", quote(label)))
    }

    pub fn nth(&self, i: usize) -> Locator<'a> {
        self.locator(&format!("nth={i}"))
    }

    pub fn first(&self) -> Locator<'a> {
        self.nth(0)
    }

    fn act(&self, method: &str, extra: Value) -> Result<Value, Error> {
        let mut p = json!({"selector": self.selector});
        if let (Value::Object(p), Value::Object(e)) = (&mut p, extra) {
            p.extend(e);
        }
        self.d.call(method, p)
    }

    pub fn click(&self) -> Result<Value, Error> {
        self.act("input.click", json!({}))
    }

    pub fn hover(&self) -> Result<Value, Error> {
        self.act("input.hover", json!({}))
    }

    pub fn focus(&self) -> Result<Value, Error> {
        self.act("input.focus", json!({}))
    }

    /// SV51 `input.pointer_move`: `params` takes `at` (`[x, y]`, the node's local points; default
    /// its centre) and `modifiers` (`"Shift+Ctrl"`).
    pub fn pointer_move(&self, params: Value) -> Result<Value, Error> {
        self.act("input.pointer_move", params)
    }

    /// SV51 `input.wheel`: `params` takes `dx`, `dy` (egui's direction: positive `dy` scrolls up),
    /// `unit` (`line`, `pixel`, `page`; default `line`), `at`, `modifiers`.
    pub fn wheel(&self, params: Value) -> Result<Value, Error> {
        self.act("input.wheel", params)
    }

    /// SV51 `input.drag`: `params` takes `from` (`[x, y]`, default the centre), one of `to`
    /// (`[x, y]` in the node's local points), `to_selector` or `to_ref`, `steps` (default 10),
    /// `button` (`primary`, `secondary`, `middle`), `modifiers`.
    pub fn drag(&self, params: Value) -> Result<Value, Error> {
        self.act("input.drag", params)
    }

    pub fn fill(&self, text: &str) -> Result<Value, Error> {
        self.act("input.fill", json!({"text": text}))
    }

    pub fn set_value(&self, value: Value) -> Result<Value, Error> {
        self.act("input.set_value", json!({"value": value}))
    }

    /// AU2b: one handle of a multi-handle widget (`"low"`/`"high"` of a `range-slider`, or an index).
    pub fn set_value_handle(&self, handle: &str, value: Value) -> Result<Value, Error> {
        self.act("input.set_value", json!({"value": value, "handle": handle}))
    }

    /// Every node that matches now (no waiting).
    pub fn all(&self) -> Result<Vec<Value>, Error> {
        Ok(self.d.call("scene.query", json!({"selector": self.selector}))?.as_array().cloned().unwrap_or_default())
    }

    pub fn count(&self) -> Result<usize, Error> {
        Ok(self.all()?.len())
    }

    /// The one node (strict).
    pub fn info(&self) -> Result<Value, Error> {
        self.d.call("scene.node", json!({"selector": self.selector}))
    }

    pub fn text_content(&self) -> Result<Option<String>, Error> {
        Ok(self.info()?["text"].as_str().map(str::to_string))
    }

    /// The scene snapshot of this locator's nodes.
    pub fn snapshot(&self) -> Result<String, Error> {
        Ok(self.d.call("scene.snapshot", json!({"selector": self.selector}))?["text"].as_str().unwrap_or_default().to_string())
    }
}

/// Web-first assertions: each is one `wait.for` the app evaluates after every settle, with the
/// timeout in virtual ms (default 5000).
pub fn expect<'a>(locator: Locator<'a>) -> Expectation<'a> {
    Expectation { locator, timeout: 5000.0 }
}

pub struct Expectation<'a> {
    locator: Locator<'a>,
    timeout: f64,
}

impl Expectation<'_> {
    pub fn timeout(mut self, ms: f64) -> Self {
        self.timeout = ms;
        self
    }

    fn wait(&self, state: &str, expect: Value) -> TestResult {
        self.locator
            .d
            .call("wait.for", json!({"selector": self.locator.selector, "state": state, "expect": expect, "timeout": self.timeout}))
            .map(drop)
    }

    pub fn to_be_visible(&self) -> TestResult {
        self.wait("visible", Value::Null)
    }

    pub fn to_be_hidden(&self) -> TestResult {
        self.wait("hidden", Value::Null)
    }

    pub fn to_be_attached(&self) -> TestResult {
        self.wait("attached", Value::Null)
    }

    pub fn to_be_detached(&self) -> TestResult {
        self.wait("detached", Value::Null)
    }

    pub fn to_be_enabled(&self) -> TestResult {
        self.wait("enabled", Value::Null)
    }

    pub fn to_have_text(&self, text: &str) -> TestResult {
        self.wait("attached", json!({"text": text}))
    }

    pub fn to_contain_text(&self, text: &str) -> TestResult {
        self.wait("attached", json!({"text": text, "exact": false}))
    }

    pub fn to_have_value(&self, value: &str) -> TestResult {
        self.wait("attached", json!({"value": value}))
    }

    pub fn to_have_count(&self, n: usize) -> TestResult {
        self.wait("attached", json!({"count": n}))
    }

    /// Settles, then compares the locator's scene snapshot with `<snapshot dir>/<name>.yaml`
    /// (indentation ignored). A missing file, or `SVELTERS_UPDATE_SNAPSHOTS=1`, writes it.
    pub fn to_match_scene_snapshot(&self, name: &str) -> TestResult {
        let d = self.locator.d;
        d.call("wait.settled", json!({"timeout": self.timeout}))?;
        let got = if self.locator.selector.is_empty() { d.snapshot()? } else { self.locator.snapshot()? };
        let path = d.snapshots.join(format!("{name}.yaml"));
        let update = std::env::var("SVELTERS_UPDATE_SNAPSHOTS").is_ok_and(|v| v == "1");
        let norm = |s: &str| s.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n");
        match std::fs::read_to_string(&path) {
            Ok(want) if !update => {
                if norm(&want) == norm(&got) {
                    Ok(())
                } else {
                    Err(Error::new("expect.snapshot", format!("{} differs (SVELTERS_UPDATE_SNAPSHOTS=1 rewrites it)\n--- want\n{want}\n--- got\n{got}", path.display())))
                }
            }
            _ => {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir).map_err(setup)?;
                }
                std::fs::write(&path, &got).map_err(setup)
            }
        }
    }
}

fn env_u64(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.trim().parse().ok())
}

/// `svelters-run` for out-of-process tests (see the module docs).
fn run_bin(spec: &TestSpec) -> Result<PathBuf, Error> {
    if let Ok(p) = std::env::var("SVELTERS_RUN") {
        return Ok(PathBuf::from(p));
    }
    if let Some(p) = spec.run_bin {
        return Ok(PathBuf::from(p));
    }
    let exe = std::env::current_exe().map_err(setup)?;
    // target/<profile>/deps/<test> → target/<profile>/svelters-run
    if let Some(dir) = exe.parent().and_then(Path::parent) {
        let p = dir.join(format!("svelters-run{}", std::env::consts::EXE_SUFFIX));
        if p.exists() {
            return Ok(p);
        }
    }
    Err(setup("SVELTERS_E2E=process: no svelters-run found; build it (`cargo build -p svelters-automation --features run --bin svelters-run`) or set SVELTERS_RUN"))
}

/// Starts the app for one seed, in process or out of process (`SVELTERS_E2E`).
pub fn launch(spec: &TestSpec, seed: u64) -> Result<E2e, Error> {
    let clock = ClockMode::parse(spec.clock).map_err(setup)?;
    let app = Path::new(spec.manifest_dir).join(spec.app);
    let mut d = match std::env::var("SVELTERS_E2E").as_deref() {
        Ok("process") => match std::env::var("SVELTERS_APP") {
            Ok(bin) => E2e::process(Path::new(&bin), None, clock, seed, spec.step_ms)?,
            Err(_) => E2e::process(&run_bin(spec)?, Some(&app), clock, seed, spec.step_ms)?,
        },
        Ok("") | Ok("in-process") | Err(_) => E2e::in_process(&app, clock, seed, spec.step_ms)?,
        Ok(m) => return Err(setup(format!("SVELTERS_E2E={m}: in-process | process"))),
    };
    d.set_snapshot_dir(Path::new(spec.manifest_dir).join("tests/snapshots"));
    Ok(d)
}

/// The seeds a test runs: `seed..seed + iterations`, with `SEED` and `ITERATIONS` overriding.
pub fn seeds(spec: &TestSpec) -> std::ops::Range<u64> {
    let start = env_u64("SEED").unwrap_or(spec.seed);
    let n = env_u64("ITERATIONS").unwrap_or(if std::env::var("SEED").is_ok() { 1 } else { spec.iterations.max(1) });
    start..start.saturating_add(n)
}

/// Runs `body` once per seed on a fresh app (what [`crate::test`] expands to). A failure (an
/// `Err` or a panic) prints the seed and the replay command, then fails the test.
pub fn run(spec: TestSpec, body: impl Fn(&mut E2e) -> TestResult) {
    for seed in seeds(&spec) {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut d = launch(&spec, seed)?;
            body(&mut d)
        }));
        let failure = match outcome {
            Ok(Ok(())) => continue,
            Ok(Err(e)) => e.to_string(),
            Err(p) => p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panicked".into()),
        };
        let mode = std::env::var("SVELTERS_E2E").unwrap_or_else(|_| "in-process".into());
        eprintln!("{}: FAILED with SEED={seed} ({mode})\n  replay: SEED={seed} ITERATIONS=1 cargo test {}", spec.name, spec.name);
        panic!("{} (SEED={seed}): {failure}", spec.name);
    }
}
