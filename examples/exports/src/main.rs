//! `exports`: runs `app/dist/app.js` on GTK with the `svelters:app/demo` exports of this crate.
//!
//! ```text
//! exports                      # debug builds first refresh app/.svelters/app (typings + stubs)
//! exports --dump <dir>         # write exports.json, app.d.ts and the stubs to <dir> and exit
//! exports --bundle <app.js>
//! ```

use std::path::PathBuf;
use std::process::ExitCode;
use svelters_abi::BackendRef;
use svelters_host::{Host, HostConfig};

fn main() -> ExitCode {
    exports_demo::link();
    let mut args = std::env::args().skip(1);
    let mut bundle = exports_demo::bundle();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--dump" => {
                let dir = PathBuf::from(args.next().unwrap_or_else(|| ".svelters/app".into()));
                return match svelters::exports::dump(&dir) {
                    Ok(r) => {
                        println!("{}: {} written, {} unchanged", dir.display(), r.written.len(), r.unchanged);
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("dump: {e}");
                        ExitCode::FAILURE
                    }
                };
            }
            "--bundle" => bundle = PathBuf::from(args.next().unwrap_or_default()),
            other => {
                eprintln!("unknown argument {other}\nusage: exports [--dump <dir>] [--bundle <app.js>]");
                return ExitCode::from(2);
            }
        }
    }
    // A debug host keeps the typings and stubs in step with this binary.
    if let Ok(r) = svelters::exports::dump_if_debug(&exports_demo::exports_dir()) {
        if !r.written.is_empty() {
            eprintln!("[exports] refreshed {} file(s) in {}: rebuild the app (pnpm build) to pick them up", r.written.len(), exports_demo::exports_dir().display());
        }
    }
    let backend = match svelters_gtk::GtkBackend::new() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("gtk: {e}");
            return ExitCode::FAILURE;
        }
    };
    let br = unsafe { BackendRef::from_raw(backend.as_raw()) }.expect("backend");
    let host = match Host::new(br, HostConfig::default()) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = host.load_bundle(&bundle) {
        eprintln!("{e}");
        return ExitCode::FAILURE;
    }
    let code = match host.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    };
    // Shutdown order (design note §8): the JS runtime first, then the services (the telemetry sampler
    // started with the first host), then the toolkit.
    drop(host);
    let report = svelters::exports::shutdown_services(std::time::Duration::from_secs(2));
    if !report.timed_out.is_empty() {
        eprintln!("[exports] services still running at exit: {:?}", report.timed_out);
    }
    code
}
