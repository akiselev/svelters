//! AU2: the out-of-process transport against the reference host `svelters-run` (headless).
//! E2E-INSPECTOR §5's AU2 acceptance: the same body gives the same YAML snapshot in process and
//! out of process; a crashed app fails alone and a fresh one passes; `app.interrupt` from the IO
//! thread breaks a JS infinite loop.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use svelters_automation::Session;
use svelters_automation::client::App;
use svelters_automation::server::Endpoint;
use svelters_egui::harness::TestApp;

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/counter-egui")
        .join(rel)
}

fn endpoint(name: &str) -> Endpoint {
    #[cfg(unix)]
    {
        // macOS caps socket paths at 104 bytes: keep it short.
        let dir = std::env::temp_dir().join(format!("sau2-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Endpoint::Unix(dir.join("a.sock"))
    }
    #[cfg(not(unix))]
    {
        let _ = name;
        Endpoint::Tcp(0)
    }
}

fn spawn(app: &Path, name: &str) -> App {
    let bin = Path::new(env!("CARGO_BIN_EXE_svelters-run"));
    App::spawn(
        bin,
        &[
            app.to_str().unwrap(),
            "--headless",
            "--clock",
            "virtual",
            "--seed",
            "3",
        ],
        &endpoint(name),
        Duration::from_secs(120),
    )
    .unwrap()
}

/// The one test body, as JSON calls: in process it gets `Session::call`, out of process
/// `Client::call`.
fn body(call: &mut dyn FnMut(&str, Value) -> Result<Value, svelters_automation::Error>) -> String {
    for _ in 0..2 {
        call(
            "input.click",
            json!({"selector": "role=button[name=\"Increment\"]"}),
        )
        .unwrap();
    }
    call(
        "input.click",
        json!({"selector": "role=button[name=\"Increment\"]", "tier": "semantic"}),
    )
    .unwrap();
    call(
        "wait.for",
        json!({"selector": "text=\"Count: 3\"", "state": "visible", "timeout": 5000}),
    )
    .unwrap();
    let snap = call("scene.snapshot", json!({})).unwrap();
    snap["text"].as_str().unwrap().to_string()
}

#[test]
fn same_body_same_snapshot_in_and_out_of_process() {
    let mut s = Session::for_test(TestApp::from_source_stepped(fixture(""), 0.016).unwrap());
    s.wait_settled(10_000.0).unwrap();
    let in_process = body(&mut |m, p| s.call(m, &p));

    let mut app = spawn(&fixture(""), "same");
    let hello = app
        .call("session.hello", &json!({"client": "ipc.rs"}))
        .unwrap();
    assert_eq!(hello["protocol"], json!(1));
    assert_eq!(hello["app"]["seed"], json!(3));
    assert_eq!(hello["app"]["headless"], json!(true));
    let out_of_process = body(&mut |m, p| app.call(m, &p));
    assert_eq!(
        out_of_process, in_process,
        "the same YAML snapshot both ways"
    );
    assert!(in_process.contains("Count: 3"), "{in_process}");

    // Errors keep their protocol codes over the wire.
    let e = app
        .call(
            "scene.locate",
            &json!({"selector": "role=button[name=\"Nope\"]"}),
        )
        .unwrap_err();
    assert_eq!(e.code, "node.not_found");
    assert_eq!(
        app.call("scene.nope", &json!({})).unwrap_err().code,
        "method.unknown"
    );

    // `app.exit` replies, then the process ends on its own.
    app.call("app.exit", &json!({})).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.alive() {
        assert!(Instant::now() < deadline, "svelters-run did not exit");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_crashed_app_fails_alone_and_the_next_one_passes() {
    let mut app = spawn(&fixture(""), "crash");
    app.call("wait.settled", &json!({"timeout": 5000})).unwrap();
    app.child.kill().unwrap();
    let _ = app.child.wait();
    let e = app
        .call("scene.query", &json!({"selector": "button"}))
        .unwrap_err();
    assert_eq!(e.code, "transport.closed", "{e}");
    drop(app);

    let mut next = spawn(&fixture(""), "crash-next");
    next.call("input.click", &json!({"selector": "text=Increment"}))
        .unwrap();
    next.call(
        "wait.for",
        &json!({"selector": "text=\"Count: 1\"", "timeout": 5000}),
    )
    .unwrap();
}

#[test]
fn interrupt_breaks_a_runaway_loop_from_the_io_thread() {
    let mut app = spawn(&fixture("src/Hang.svelte"), "hang");
    let mut side = app.connect().unwrap();
    let h = side.call("app.health", &json!({})).unwrap();
    assert_eq!(h["running"], Value::Null, "{h}");

    // The click's handler never returns: the UI thread is stuck inside it.
    let mut stuck = app.connect().unwrap();
    let click = std::thread::spawn(move || {
        stuck.call(
            "input.click",
            &json!({"selector": "text=Spin", "tier": "semantic"}),
        )
    });

    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let h = side.call("app.health", &json!({})).unwrap();
        if h["running"]["method"] == json!("input.click")
            && h["running"]["for_ms"].as_f64().unwrap() > 200.0
        {
            break;
        }
        assert!(Instant::now() < deadline, "the click never hung: {h}");
        std::thread::sleep(Duration::from_millis(20));
    }
    let r = side.call("app.interrupt", &json!({})).unwrap();
    assert_eq!(r["interrupted"], json!(true));
    assert_eq!(r["running"], json!("input.click"));
    // The click returns (its handler threw); the app answers again and still works.
    let _ = click.join().unwrap();
    app.call(
        "input.click",
        &json!({"selector": "role=button[name=\"Bump\"]"}),
    )
    .unwrap();
    app.call(
        "wait.for",
        &json!({"selector": "text=\"Bumps: 1\"", "timeout": 5000}),
    )
    .unwrap();
    let h = side.call("app.health", &json!({})).unwrap();
    assert_eq!(h["running"], Value::Null, "{h}");
    assert!(h["served"].as_u64().unwrap() >= 3, "{h}");
}
