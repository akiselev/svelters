//! Runs a compiled bundle in an eframe window: `svelters-egui-run <dist/app.js>`.
//!
//! `--idle-report <ms>`: after the app has settled (1.5 s), counts the frames drawn and the CPU time
//! the process used over `<ms>`, prints one JSON line (`{"frames":..,"cpu_ms":..,"wall_ms":..}`) and
//! exits. That is the idle-CPU measurement of the G1 milestone (run under tools/dev/headless).

use std::time::{Duration, Instant};

/// utime + stime of this process, in milliseconds (Linux).
fn cpu_ms() -> Option<f64> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let rest = &stat[stat.rfind(')')? + 2..];
    let f: Vec<&str> = rest.split(' ').collect();
    let ticks: f64 = f.get(11)?.parse::<f64>().ok()? + f.get(12)?.parse::<f64>().ok()?;
    Some(ticks * 1000.0 / 100.0) // USER_HZ is 100 on Linux
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut bundle = None;
    let mut idle_ms = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--idle-report" => idle_ms = it.next().and_then(|v| v.parse::<u64>().ok()),
            _ => bundle = Some(std::path::PathBuf::from(a)),
        }
    }
    let Some(bundle) = bundle else {
        eprintln!("usage: svelters-egui-run [--idle-report <ms>] <dist/app.js>");
        std::process::exit(2);
    };
    let backend = svelters_egui::EguiBackend::new();
    let vt = backend.vtable();
    let host = svelters_host::Host::new(vt.backend_ref(), svelters_host::HostConfig::default()).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    if let Err(e) = host.load_bundle(&bundle) {
        eprintln!("{e}");
        std::process::exit(1);
    }
    if let Some(ms) = idle_ms {
        let frames = backend.frame_counter();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1500));
            let (f0, c0, t0) = (frames(), cpu_ms().unwrap_or(f64::NAN), Instant::now());
            std::thread::sleep(Duration::from_millis(ms));
            let (f1, c1) = (frames(), cpu_ms().unwrap_or(f64::NAN));
            println!(
                "{{\"frames_before\":{f0},\"frames\":{},\"cpu_ms\":{:.1},\"wall_ms\":{}}}",
                f1 - f0,
                c1 - c0,
                t0.elapsed().as_millis()
            );
            std::process::exit(if f0 > 0 { 0 } else { 3 });
        });
    }
    if let Err(e) = host.run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
