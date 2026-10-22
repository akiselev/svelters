//! `egui-kinds`: runs `app/dist/app.js` on the egui backend with this crate's `<quantity-field>`.
//!
//! ```text
//! egui-kinds                  # debug builds first refresh app/.svelters/app (typings, kinds.json)
//! egui-kinds --dump <dir>     # write exports.json, app.d.ts, kinds.json, kinds.d.ts to <dir> and exit
//! egui-kinds --bundle <app.js>
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    egui_kinds_demo::link();
    let mut args = std::env::args().skip(1);
    let mut bundle = egui_kinds_demo::bundle();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--dump" => {
                let dir = PathBuf::from(args.next().unwrap_or_else(|| ".svelters/app".into()));
                return match svelters_egui::dump(&dir) {
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
                eprintln!("unknown argument {other}\nusage: egui-kinds [--dump <dir>] [--bundle <app.js>]");
                return ExitCode::from(2);
            }
        }
    }
    if cfg!(debug_assertions)
        && let Ok(r) = svelters_egui::dump(&egui_kinds_demo::dump_dir())
        && !r.written.is_empty()
    {
        eprintln!("[egui-kinds] refreshed {} file(s): rebuild the app (pnpm build) to pick them up", r.written.len());
    }
    match svelters_egui::run_app(&bundle, svelters_host::HostConfig::default()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
