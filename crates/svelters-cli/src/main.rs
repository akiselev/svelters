use std::io::{IsTerminal, Write};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ctx = svelters_cli::Ctx {
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        env_packs: std::env::var_os("SVELTERS_DOCS_PACK")
            .map(|v| std::env::split_paths(&v).collect())
            .unwrap_or_default(),
        explicit_only: false,
        color: std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
    };
    let out = svelters_cli::run(&args, &ctx);
    // a closed pipe (`svelters docs … | head`) is not an error
    let _ = std::io::stdout().write_all(out.stdout.as_bytes());
    let _ = std::io::stderr().write_all(out.stderr.as_bytes());
    std::process::exit(out.code);
}
