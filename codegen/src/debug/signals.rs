use anyhow::Result;

use crate::{
    helpers::read_schema,
    opts,
    types::Visitor,
    visitors::{print, signals},
};

pub fn debug_signals(cmd: &opts::Signals) -> Result<()> {
    let schema = read_schema()?;
    if cmd.simple {
        let mut classes = signals::GetSimpleSignals::default();
        classes.iter_class(schema.values());
        for class in classes.signals.iter() {
            println!("{}", class);
            for method in &class.methods {
                println!("    {}", method);
            }
        }
    } else {
        print::Signals::default().iter_class(schema.values());
    }
    Ok(())
}

pub fn debug_signal_arg_types(opts: &opts::Args) -> Result<()> {
    let schema = read_schema()?;
    let mut args = signals::ArgTypes::default();
    args.iter_class(schema.values());
    let items: Vec<_> = if opts.primitives {
        args.types
            .iter()
            .filter(|arg| !arg.starts_with("Q"))
            .collect()
    } else if opts.classes {
        args.types
            .iter()
            .filter(|arg| arg.starts_with("Q"))
            .collect()
    } else {
        args.types.iter().collect()
    };
    for item in items {
        println!("{}", item);
    }
    Ok(())
}
