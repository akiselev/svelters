use rquickjs::{Loader, Resolver};

use crate::inner::*;

pub(in crate) fn get_dev_dependencies() -> (impl Resolver, impl Loader) {
    let resolver = (
        BuiltinResolver::default()
            .with_module("svelte/internal")
            .with_module("./internal/index.mjs")
            .with_module("svelte"),
        FileResolver::default()
            .with_path("./")
            .with_path("./node_modules/svelte/"),
    );
    let loader = (
        BuiltinLoader::default()
            .with_module(
                "svelte/internal",
                std::fs::read_to_string("./node_modules/svelte/internal/index.mjs").unwrap(),
            )
            .with_module(
                "./internal/index.mjs",
                std::fs::read_to_string("./node_modules/svelte/internal/index.mjs").unwrap(),
            )
            .with_module(
                "svelte",
                std::fs::read_to_string("./node_modules/svelte/index.mjs").unwrap(),
            ),
        ScriptLoader::default(),
        NativeLoader::default(),
    );

    (resolver, loader)
}

pub(in crate) fn get_prod_dependencies() -> (impl Resolver, impl Loader) {
    let resolver = (
        BuiltinResolver::default()
            .with_module("svelte/internal")
            .with_module("./internal/index.mjs")
            .with_module("svelte"),
        FileResolver::default()
            .with_path("./")
            .with_path("./node_modules/svelte/"),
    );
    let loader = (
        BuiltinLoader::default()
            .with_module(
                "svelte/internal",
                include_str!("../../../node_modules/svelte/internal/index.mjs"),
            )
            .with_module(
                "./internal/index.mjs",
                include_str!("../../../node_modules/svelte/internal/index.mjs"),
            )
            .with_module(
                "svelte",
                include_str!("../../../node_modules/svelte/index.mjs"),
            ),
        ScriptLoader::default(),
        NativeLoader::default(),
    );

    (resolver, loader)
}
