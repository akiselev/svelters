use node_resolve::Resolver as NodeResolver;
use rquickjs::{Ctx, Resolver};

use crate::inner::*;

impl Resolver for SvelteCompiler {
    fn resolve<'js>(&mut self, ctx: Ctx<'js>, base: &str, name: &str) -> Result<String> {
        tracing::info!(%base, %name, "Resolving {} from {}",  name, base);
        let base = match std::fs::canonicalize(base) {
            Ok(path) if path.is_file() => path.parent().unwrap().to_owned(),
            Ok(path) => path,
            Err(_) => std::fs::canonicalize("./").unwrap(),
        };
        let resolver = self.resolver.with_basedir(base.clone().into());
        resolver
            .resolve(name)
            .map(|buf| buf.to_str().unwrap().to_string())
            .map_err(|err| rquickjs::Error::Resolving {
                base: base.to_string_lossy().into(),
                name: name.to_string(),
                message: Some(err.to_string()),
            })
            .map(|res| {
                tracing::info!(?base, %name, "Resolved to {}",  res);
                res
            })
    }
}
