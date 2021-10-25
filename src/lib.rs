#![feature(generic_associated_types)]
// Allows impl Trait in associated type
#![feature(type_alias_impl_trait)]
#![feature(const_impl_trait)]
// Features for evaluating expressions in const generics
// #![feature(const_evaluatable_checked)]
// Allows const str from utf8 bytes
// #![feature(const_str_from_utf8_unchecked)]
#![feature(const_fn_trait_bound)]
#![feature(const_type_name)]
#![feature(specialization)]
#![allow(incomplete_features)]
#![recursion_limit = "1024"]
#![deny(clippy::all)]
use std::{
    ffi::CString,
    os::raw::c_char,
    path::{Path, PathBuf},
    time::Duration,
};

use rquickjs::{
    bind, BuiltinLoader, BuiltinResolver, Context, Ctx, ExecutorSpawner, FileResolver, FromJs,
    Func, Function, IntoAtom, IntoJs, ModuleLoader, NativeLoader, Object, Persistent,
    Result as JsResult, Runtime as JsRuntime, ScriptLoader, Value,
};

use svelters_compiler::{compile_sync, SvelteCompiler};
use svelters_core::prelude::*;

pub mod logging;

#[tracing::instrument(skip(svelters, compiler))]
pub fn eval_file<P>(svelters: &Svelters, compiler: &SvelteCompiler, path: P) -> Result<()>
where
    P: std::fmt::Debug + AsRef<Path>,
{
    let modpath = path.as_ref().to_owned();
    let src = compile_sync(compiler, path)?;

    let res: Result<()> = svelters.with_ctx(|ctx| {
        ctx.with(|ctx| {
            let res = ctx.eval_global_module::<Value, _>(modpath, Some(src.into_bytes()));
            match res {
                Ok(ok) => {
                    tracing::info!("Success! Result: {:?}", ok);
                    Ok(())
                }
                Err(err) => {
                    tracing::error!("Failure! Result: {}", err);
                    Err(err)
                }
            }
        })
    });
    res
}

pub struct CompiledSource {
    pub path: PathBuf,
    pub source: String,
}

#[tracing::instrument(skip(compiler))]
pub fn compile_file<P>(compiler: &SvelteCompiler, path: P) -> Result<CompiledSource>
where
    P: std::fmt::Debug + AsRef<Path>,
{
    let modpath = path.as_ref().to_owned();
    let source = compile_sync(compiler, path)?;
    return Ok(CompiledSource {
        path: modpath,
        source,
    });
}
