#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;

pub mod code;
pub mod commands;
pub mod constants;
pub mod debug;
pub mod helpers;
pub mod opts;
pub mod types;
pub mod visitors;

use crate::{
    commands::{generate::generate, schema::run_codegen},
    debug::signals::{debug_signal_arg_types, debug_signals},
    opts::{Command, DebugCommand},
};

fn main() {
    match &opts::OPTS.cmd {
        Command::Schema(cmd) => run_codegen(cmd).unwrap(),
        Command::Generate(cmd) => generate(cmd).unwrap(),
        Command::Debug(cmd) => match &cmd.cmd {
            DebugCommand::Signals(subcmd) => debug_signals(subcmd).unwrap(),
            DebugCommand::Args(subcmd) => debug_signal_arg_types(subcmd).unwrap(),
        },
        _ => unimplemented!(),
    }
}
