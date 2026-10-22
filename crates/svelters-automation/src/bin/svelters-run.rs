//! AU2's reference automation host (E2E-INSPECTOR §1.9, §2.3): runs a svelters egui app headless
//! (the kittest runner inside the binary: no window, frames only when a request asks for them,
//! time advances by `--step-ms` per frame) and serves [`Session::call`]'s methods on a local
//! socket.
//!
//! ```text
//! svelters-run [<app dir | App.svelte | dist/app.js>] --automation <unix:/path | /path | tcp:PORT> --headless
//!              [--clock virtual|real] [--seed N] [--step-ms 16]
//! ```
//!
//! AU1b: `--clock virtual` (the default) runs the host on a seeded [`VirtualClock`] (timers,
//! `performance.now()`, worker jobs in a seeded order, `Math.random`), advanced one `--step-ms` per
//! frame; without `--seed` the seed is 0. `--clock real` keeps the wall clock and paces frames in
//! wall time; `--seed` then seeds only `Math.random`. Windowed mode is not here: a windowed app
//! mounts [`Server::pump`] in its own frame loop.
//!
//! It prints `SVELTERS_AUTOMATION=<endpoint>` once it listens (`tcp:0` gets the real port). The
//! `--automation` flag is the opt-in: automation is off in every svelters app until the app turns
//! it on. Extra methods: `app.exit` (reply, then exit 0). `session.hello` adds an `app` object
//! (`headless`, `clock`, `seed`, `frame_ms`, `mount`).
//!
//! AU2b (the TS runner's worker-scoped app, E2E-INSPECTOR §2.2/§2.4): `app.reset {seed?}` rebuilds
//! the start app in a fresh host on a fresh clock; `app.mount {module, export?, props?, size?,
//! seed?}` puts one `.svelte` component (compiled live) alone in a fresh host on a fresh clock;
//! `app.set_props {props}` shallow-merges props into the mounted root. Each rebuild opens a new
//! session (every grant) whose epoch is past the old one's. Without `<app>` the host starts empty,
//! waiting for `app.mount`.

use serde_json::{Value, json};
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use svelters_automation::clock::{ClockMode, Clocked, RealTime, VirtualClock};
use svelters_automation::server::{Endpoint, Server};
use svelters_automation::{Driver, Error, Grants, Session};
use svelters_egui::embed::{Embed, Props, RootId};
use svelters_egui::harness::TestApp;
use svelters_host::{HostConfig, HostSched};

struct Args {
    app: Option<String>,
    automation: Endpoint,
    clock: ClockMode,
    seed: Option<u64>,
    step_ms: f32,
}

const USAGE: &str = "usage: svelters-run [<app>] --automation <endpoint> --headless [--clock virtual|real] [--seed N] [--step-ms MS]";

fn args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let (mut app, mut automation, mut headless, mut clock, mut seed, mut step_ms) =
        (None, None, false, ClockMode::Virtual, None, 16.0f32);
    while let Some(a) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .ok_or_else(|| format!("{name} wants a value\n{USAGE}"))
        };
        match a.as_str() {
            "--automation" => {
                automation =
                    Some(Endpoint::parse(&value("--automation")?).map_err(|e| e.to_string())?)
            }
            "--headless" => headless = true,
            "--clock" => clock = ClockMode::parse(&value("--clock")?)?,
            "--seed" => {
                seed = Some(
                    value("--seed")?
                        .parse()
                        .map_err(|_| "--seed wants an integer".to_string())?,
                )
            }
            "--step-ms" => {
                step_ms = value("--step-ms")?
                    .parse()
                    .map_err(|_| "--step-ms wants a number".to_string())?
            }
            "-h" | "--help" => return Err(USAGE.into()),
            s if s.starts_with('-') => return Err(format!("unknown flag {s}\n{USAGE}")),
            s => app = Some(s.to_string()),
        }
    }
    if !headless {
        return Err(format!(
            "pass --headless: svelters-run has only the headless runner (a windowed app mounts svelters_automation::server::Server in its own frame loop)\n{USAGE}"
        ));
    }
    Ok(Args {
        app,
        automation: automation
            .ok_or_else(|| format!("--automation <endpoint> is required\n{USAGE}"))?,
        clock,
        seed,
        step_ms,
    })
}

/// Compiled live from source, or a built bundle (`.js`).
fn test_app(app: &str, dt: f32, cfg: HostConfig) -> Result<TestApp, String> {
    if app.ends_with(".js") {
        TestApp::load_with(app, dt, cfg)
    } else {
        TestApp::from_source_with(app, dt, cfg)
    }
}

/// What the host runs: the app it was started with, a mounted component (`app.mount`), or (no
/// app given) an empty embed host waiting for `app.mount`.
#[derive(Clone)]
enum Content {
    App(String),
    Mount {
        module: String,
        export: Option<String>,
        props: Value,
        size: [f32; 2],
    },
    Empty,
}

/// One build of the host: the driver on a fresh clock, the interrupt flag and, for a mount, the
/// embed and root that `app.set_props` patches.
struct Built {
    driver: Box<dyn Driver>,
    interrupt: Arc<AtomicBool>,
    mounted: Option<(Embed, RootId)>,
}

/// Builds `content` on a fresh host. AU2b: every build gets a new [`VirtualClock`] (it cannot be
/// reset in place) whose [`VirtualClock::config`] gives the host its spawner, clock and seed, so a
/// reset or a mount replays exactly like a fresh `svelters-run --seed N`.
fn build(
    content: &Content,
    mode: ClockMode,
    seed: Option<u64>,
    step_ms: f32,
) -> Result<Built, String> {
    let quiet = HostConfig {
        echo_console: false,
        echo_errors: false,
        ..HostConfig::default()
    };
    let dt = step_ms / 1000.0;
    let make = |cfg: HostConfig| -> Result<(TestApp, Option<RootId>), String> {
        match content {
            Content::App(app) => Ok((test_app(app, dt, cfg)?, None)),
            Content::Mount {
                module,
                export,
                props,
                size,
            } => {
                let (app, root) = TestApp::mount_source(
                    module,
                    export.as_deref(),
                    props.clone(),
                    egui::vec2(size[0], size[1]),
                    dt,
                    cfg,
                )?;
                Ok((app, Some(root)))
            }
            Content::Empty => Ok((
                TestApp::embed_host(egui::vec2(480.0, 360.0), dt, cfg)?.0,
                None,
            )),
        }
    };
    let (driver, app_parts): (Box<dyn Driver>, _) = match mode {
        ClockMode::Virtual => {
            let clock = VirtualClock::new(seed.unwrap_or(0));
            let (app, root) = make(clock.config(quiet))?;
            let parts = (
                app.host().interrupt_flag(),
                root.map(|r| (app.embed.clone(), r)),
            );
            (Box::new(Clocked::new(app, clock)), parts)
        }
        ClockMode::Real => {
            let sched = match seed {
                Some(s) => HostSched::default().seed(s),
                None => HostSched::default(),
            };
            let (app, root) = make(HostConfig { sched, ..quiet })?;
            let parts = (
                app.host().interrupt_flag(),
                root.map(|r| (app.embed.clone(), r)),
            );
            (Box::new(RealTime::new(app)), parts)
        }
    };
    Ok(Built {
        driver,
        interrupt: app_parts.0,
        mounted: app_parts.1,
    })
}

fn rpc_err(e: String) -> Error {
    Error::new("app.load", e)
}

/// `{seed}` of `app.reset`/`app.mount`: the given seed, else the one the host was started with.
fn seed_param(p: &Value, default: Option<u64>) -> Result<Option<u64>, Error> {
    match p.get("seed") {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_u64()
            .map(Some)
            .ok_or_else(|| Error::new("params.invalid", "seed: want a non-negative integer")),
    }
}

/// Replaces the session's driver with `built`: a new session (every grant, as at start), its epoch
/// past the old one's so refs taken before the swap read as stale. The first settle is the
/// client's to wait for.
fn swap(
    s: &mut Session<Box<dyn Driver>>,
    built: Built,
    server: &Server,
) -> Result<Option<(Embed, RootId)>, Error> {
    let epoch = s.epoch();
    let mut next = Session::new(built.driver, Grants::ALL)?;
    for _ in 0..=epoch {
        next.bump_epoch();
    }
    *s = next;
    server.set_interrupt(built.interrupt);
    Ok(built.mounted)
}

fn run() -> Result<(), String> {
    let a = args()?;
    // `--automation` is this process's opt-in.
    svelters_automation::enable();
    let mut content = match &a.app {
        Some(app) => Content::App(app.clone()),
        None => Content::Empty,
    };
    let start = content.clone();
    let mut seed = a.seed;
    let built = build(&content, a.clock, seed, a.step_ms)?;
    let interrupt = built.interrupt.clone();
    let mut mounted = built.mounted;
    let mut session = Session::new(built.driver, Grants::ALL).map_err(|e| e.to_string())?;
    // A slow first settle is the client's to see (`wait.settled`), not a start failure.
    let _ = session.wait_settled(10_000.0);
    let server = Server::start(&session, a.automation).map_err(|e| e.to_string())?;
    server.set_interrupt(interrupt);
    {
        let mut out = std::io::stdout().lock();
        writeln!(out, "SVELTERS_AUTOMATION={}", server.endpoint())
            .and_then(|_| out.flush())
            .map_err(|e| e.to_string())?;
    }
    let mut exit = false;
    while !exit {
        server.pump_session(
            Some(Duration::from_millis(200)),
            &mut |s: &mut Session<Box<dyn Driver>>, m: &str, p: &Value| match m {
                "app.exit" => {
                    exit = true;
                    Ok(json!({}))
                }
                "session.hello" => {
                    let mut v = s.call(m, p)?;
                    v["app"] = json!({"headless": true, "clock": a.clock.as_str(), "seed": s.driver().seed().or(seed), "frame_ms": s.driver().frame_ms(), "mount": true});
                    Ok(v)
                }
                // AU2b: back to the app the host was started with, on a fresh host and clock.
                "app.reset" => {
                    seed = seed_param(p, a.seed)?;
                    content = start.clone();
                    let built = build(&content, a.clock, seed, a.step_ms).map_err(rpc_err)?;
                    mounted = swap(s, built, &server)?;
                    let frames = s.wait_settled(timeout_param(p))?;
                    Ok(json!({"seed": seed, "frames": frames}))
                }
                // AU2b (§2.4): one component alone in a fresh host: `{module, export?, props?,
                // size?: [w, h], seed?}`. `module` is a `.svelte` path inside an app.
                "app.mount" => {
                    let module = p
                        .get("module")
                        .and_then(Value::as_str)
                        .ok_or_else(|| Error::new("params.invalid", "module: want a path"))?;
                    let size = match p.get("size").and_then(Value::as_array) {
                        Some(v) if v.len() == 2 => [
                            v[0].as_f64().unwrap_or(480.0) as f32,
                            v[1].as_f64().unwrap_or(360.0) as f32,
                        ],
                        _ => [480.0, 360.0],
                    };
                    seed = seed_param(p, a.seed)?;
                    content = Content::Mount {
                        module: module.to_string(),
                        export: p.get("export").and_then(Value::as_str).map(str::to_string),
                        props: p.get("props").cloned().unwrap_or_else(|| json!({})),
                        size,
                    };
                    let built = build(&content, a.clock, seed, a.step_ms).map_err(rpc_err)?;
                    mounted = swap(s, built, &server)?;
                    let frames = s.wait_settled(timeout_param(p))?;
                    Ok(json!({"root": "mount", "seed": seed, "frames": frames}))
                }
                // Shallow-merges `{props}` into the mounted root's props, then settles.
                "app.set_props" => {
                    let (embed, root) = mounted
                        .as_ref()
                        .ok_or_else(|| Error::new("capability.absent", "app.set_props: nothing is mounted (app.mount first)"))?;
                    embed
                        .set_root_props(*root, Props::json(p.get("props").cloned().unwrap_or_default()))
                        .map_err(|e| Error::new("app.load", e.to_string()))?;
                    let frames = s.wait_settled(timeout_param(p))?;
                    Ok(json!({"frames": frames}))
                }
                _ => s.call(m, p),
            },
            &mut session,
        );
    }
    // Let the writer thread send `app.exit`'s reply before the process ends.
    drop(server);
    std::thread::sleep(Duration::from_millis(50));
    Ok(())
}

fn timeout_param(p: &Value) -> f64 {
    p.get("timeout").and_then(Value::as_f64).unwrap_or(10_000.0)
}

fn main() {
    if let Err(e) = run() {
        eprintln!("svelters-run: {e}");
        std::process::exit(2);
    }
}
