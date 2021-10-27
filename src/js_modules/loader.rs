use node_resolve::Resolver as NodeResolver;
use rquickjs::{BuiltinResolver, Loader, ModuleDef, ModuleLoader, Resolver};

use crate::js::*;

pub struct SvResolver {
    pub(in crate) resolver: NodeResolver,
}

impl SvResolver {
    pub fn new() -> SvResolver {
        let resolver = NodeResolver::new()
            .with_extensions(&["svelte", "ts", "mjs", "js"])
            .with_main_fields(&["svelte", "module"]);

        SvResolver { resolver }
    }
}

impl Resolver for SvResolver {
    fn resolve<'js>(&mut self, ctx: Ctx<'js>, base: &str, name: &str) -> rquickjs::Result<String> {
        tracing::info!(%base, %name, "Resolving {} from {}",  name, base);
        let base = if base.starts_with("/") {
            match std::fs::canonicalize(base) {
                Ok(path) if path.is_file() => path.parent().unwrap().to_owned(),
                Ok(path) => path,
                Err(_) => std::fs::canonicalize("./").unwrap(),
            }
            .to_str()
            .unwrap()
            .to_string()
        } else {
            self.resolver
                .with_basedir("./".into())
                .resolve(base)
                .map(|buf| buf.parent().unwrap().to_str().unwrap().to_string())
                .unwrap()
        };
        let resolver = self.resolver.with_basedir(base.clone().into());
        resolver
            .resolve(name)
            .map(|buf| buf.to_str().unwrap().to_string())
            .map_err(|err| rquickjs::Error::Resolving {
                base: base.clone(),
                name: name.to_string(),
                message: Some(err.to_string()),
            })
            .map(|res| {
                tracing::info!(?base, %name, "Resolved to {}",  res);
                res
            })
    }
}
