//! AU2's reference automation host (E2E-INSPECTOR §1.9, §2.3): runs a svelters egui app headless
//! (the kittest runner inside the binary: no window, frames only when a request asks for them,
//! time advances by `--step-ms` per frame) and serves [`Session::call`]'s methods on a local
//! socket.
//!
//! ```text
//! svelters-run <app dir | App.svelte> --automation <unix:/path | /path | tcp:PORT> --headless
//!              [--clock virtual] [--seed N] [--step-ms 16]
//! ```
//!
//! It prints `SVELTERS_AUTOMATION=<endpoint>` once it listens (`tcp:0` gets the real port). The
//! `--automation` flag is the opt-in: automation is off in every svelters app until the app turns
//! it on. Extra methods: `app.exit` (reply, then exit 0). `session.hello` adds an `app` object
//! (`headless`, `clock`, `seed`, `frame_ms`).

use serde_json::{Value, json};
use std::io::Write;
use std::time::Duration;
use svelters_automation::server::{Endpoint, Server};
use svelters_automation::{Grants, Session};
use svelters_egui::harness::TestApp;

struct Args {
    app: String,
    automation: Endpoint,
    seed: Option<u64>,
    step_ms: f32,
}

const USAGE: &str = "usage: svelters-run <app> --automation <endpoint> --headless [--clock virtual] [--seed N] [--step-ms MS]";

fn args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let (mut app, mut automation, mut headless, mut seed, mut step_ms) =
        (None, None, false, None, 16.0f32);
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
            "--clock" => match value("--clock")?.as_str() {
                "virtual" => {}
                c => {
                    return Err(format!(
                        "--clock {c}: the headless runner has only the virtual clock"
                    ));
                }
            },
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
        app: app.ok_or_else(|| format!("no app given\n{USAGE}"))?,
        automation: automation
            .ok_or_else(|| format!("--automation <endpoint> is required\n{USAGE}"))?,
        seed,
        step_ms,
    })
}

fn run() -> Result<(), String> {
    let a = args()?;
    // `--automation` is this process's opt-in.
    svelters_automation::enable();
    let app = TestApp::from_source_stepped(&a.app, a.step_ms / 1000.0)?;
    let interrupt = app.host().interrupt_flag();
    let mut session = Session::new(app, Grants::ALL).map_err(|e| e.to_string())?;
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
            &mut |s: &mut Session<TestApp>, m: &str, p: &Value| match m {
                "app.exit" => {
                    exit = true;
                    Ok(json!({}))
                }
                "session.hello" => {
                    let mut v = s.call(m, p)?;
                    v["app"] = json!({"headless": true, "clock": "virtual", "seed": a.seed, "frame_ms": svelters_automation::Driver::frame_ms(s.driver())});
                    Ok(v)
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

fn main() {
    if let Err(e) = run() {
        eprintln!("svelters-run: {e}");
        std::process::exit(2);
    }
}
