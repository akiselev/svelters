use std::collections::HashMap;

use anyhow::Result;

use crate::{code::traits::codegen, helpers::read_schema, opts};

pub fn generate(cmd: &opts::Generate) -> Result<()> {
    let schema = read_schema()?;
    let schema: HashMap<_, _> = schema
        .into_iter()
        .filter(|(_, cls)| cls.inherits.contains(&"QWidget".to_string()))
        .filter(|(_, cls)| !cls.def.name.starts_with("QMac"))
        .collect();
    std::fs::write(&cmd.output, codegen(&schema)?)?;
    Ok(())
}
