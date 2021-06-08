use clap::{AppSettings, Clap};

/// Cadatomic Workspace Configuration
#[derive(Clap)]
#[clap(version = "1.0", author = "Alexander K. <alex@akiselev.com>")]
#[clap(setting = AppSettings::ColoredHelp)]
pub struct Opts {
    /// Path to Qt documentation root
    #[clap(long, default_value = "docs/qt/", env = "CODEGEN_QT_DOCS")]
    pub(crate) qt: String,

    /// Level of trace verbosity
    // #[clap(short, long, parse(from_occurrences))]
    // pub(crate) verbose: i32,

    #[clap(subcommand)]
    pub(crate) cmd: Command,

    /// Path to JSON file where codegen will store the schema
    #[clap(short, long, env = "CODEGEN_SCHEMA", default_value = "schema.json")]
    pub(crate) schema: String,
}

#[derive(Clap)]
pub enum Command {
    Schema(Schema),
    Debug(Debug),
    Generate(Generate),
}

/// Run Rust QT wrapper for Svelte
#[derive(Clap)]
pub struct Schema {}

#[derive(Clap)]
pub struct Generate {
    /// Path to JSON file where codegen will store the schema
    #[clap(short, long, env = "CODEGEN_OUTPUT", default_value = "output.rs")]
    pub(crate) output: String,
}

/// Inspect intermediate state
#[derive(Clap)]
pub struct Debug {
    #[clap(subcommand)]
    pub(crate) cmd: DebugCommand,
}

#[derive(Clap)]
pub enum DebugCommand {
    Signals(Signals),
    Args(Args),
}

#[derive(Clap)]
pub struct Args {
    /// Only show primitives (types not beginning with Q)
    #[clap(short, long)]
    pub(crate) primitives: bool,

    /// Only show Qt types (types beginning with Q)
    #[clap(short, long)]
    pub(crate) classes: bool,
}

/// A subcommand for controlling testing
#[derive(Clap)]
pub struct Signals {
    /// Only show simple signals
    #[clap(short, long)]
    pub(crate) simple: bool,
}

lazy_static! {
    pub static ref OPTS: Opts = Opts::parse();
}
