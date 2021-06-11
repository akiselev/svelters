use node_resolve::Resolver as NodeResolver;
use rquickjs::{Ctx, Resolver as JsResolver};

pub struct Resolver {
    node: NodeResolver,
}

impl Resolver {
    pub fn new() -> Resolver {
        let node = NodeResolver::new()
            .with_extensions(&["svelte", "ts", "js"])
            .with_main_fields(&["svelte", "module"]);
        Resolver { node }
    }
}

impl JsResolver for Resolver {
    fn resolve<'js>(
        &mut self,
        ctx: Ctx<'js>,
        base: &str,
        name: &str,
    ) -> Result<String, rquickjs::Error> {
        let resolver = self.node.with_basedir(base.into());
        resolver
            .resolve(name)
            .map(|buf| buf.to_str().unwrap().to_string())
            .map_err(|err| rquickjs::Error::Resolving {
                base: base.to_string(),
                name: name.to_string(),
                message: Some(err.to_string()),
            })
    }
}
