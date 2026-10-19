//! The `svelters` command line. C0: `svelters docs` (the offline docs probe) and
//! `svelters docs-pack` (build its data from the docs IR). See `plans/research/CLI.md` and
//! `plans/milestones/CL0-cli-docs.md`.

pub mod docs;
pub mod locate;
pub mod md;
pub mod out;
pub mod pack;
pub mod resolve;
pub mod text;

pub use docs::{Ctx, Output};

pub const HELP: &str = "\
svelters: build and inspect Svelte apps rendered with native toolkits

USAGE
  svelters docs [<toolkit>] [<element>[.<member>]] [--brief|--json|--all] [-k <query>]
  svelters docs-pack [--ir <dir>] [--out <dir>] [--check]
  svelters --version

`svelters docs --help` describes addresses and output formats. create/dev/build/run/doctor
arrive in C1 (plans/research/CLI.md).
";

/// Runs one command line (without the program name).
pub fn run(args: &[String], ctx: &Ctx) -> Output {
    let ok = |s: String| Output {
        code: 0,
        stdout: s,
        stderr: String::new(),
    };
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help" | "help") => ok(HELP.into()),
        Some("-V" | "--version") => ok(format!("svelters {}\n", env!("CARGO_PKG_VERSION"))),
        Some("docs") => docs::run_docs(&args[1..], ctx),
        Some("docs-pack") => docs::run_pack(&args[1..], ctx),
        Some(c) => Output {
            code: docs::EXIT_USAGE,
            stdout: String::new(),
            stderr: format!("svelters: unknown command {c:?}\n{HELP}"),
        },
    }
}
