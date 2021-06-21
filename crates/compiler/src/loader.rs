use std::{
    collections::HashMap,
    convert::TryInto,
    fs::{canonicalize, read_to_string},
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use anyhow::Result;
use rquickjs::{
    bind, generic_loader, BuiltinResolver, Context, Created, Ctx, Func, Loaded, Loader, Module,
    ModuleDef, ModuleLoader, Native, Script,
};

use crate::{compile_sync, inner::*};

impl Loader<Script> for SvelteCompiler {
    fn load<'js>(
        &mut self,
        ctx: Ctx<'js>,
        name: &str,
    ) -> Result<Module<'js, Loaded<Script>>, rquickjs::Error> {
        let src = if name.starts_with('/') {
            compile_sync(self, name)?
        } else {
            tracing::error!(%name, "{} is not an absolute path", name);
            return Err(rquickjs::Error::Loading {
                name: name.to_string(),
                message: Some(format!("{} is not an absolute path", name)),
            });
        };
        let module = Module::new(ctx, name, src).map_err(|err| rquickjs::Error::Loading {
            name: name.to_string(),
            message: Some(err.to_string()),
        })?;
        Ok(module)
    }
}

generic_loader! {
    // Without bounds and metas
    // The `Loader<Script>` trait should be implemented for `MyScriptLoader`
    SvelteCompiler: Script,
}
