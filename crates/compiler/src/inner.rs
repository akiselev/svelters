pub(in crate) use std::{path::Path, sync::Arc};

pub(in crate) use flume::{Receiver, Sender};
pub(in crate) use futures::future::Future;
pub(in crate) use node_resolve::Resolver as NodeResolver;
pub(in crate) use parking_lot::RwLock;
pub(in crate) use rquickjs::{
    bind, BuiltinLoader, BuiltinResolver, Context, Ctx, Error, FileResolver, FromJs, Function,
    IntoJs, NativeLoader, Promise, Result, Runtime, ScriptLoader, Value,
};
pub(in crate) use swc_common::{
    errors::{ColorConfig, Handler},
    input::StringInput,
    BytePos, FileLoader, FilePathMapping, SourceMap,
};
pub(in crate) use swc_ecma_codegen::{text_writer::JsWriter, Config, Emitter};
pub(in crate) use swc_ecma_parser::{lexer::Lexer, Parser as JsParser, Syntax, TsConfig};
pub(in crate) use swc_ecma_visit::FoldWith;

pub(in crate) use crate::dependencies::{get_dev_dependencies, get_prod_dependencies};
pub(in crate) use crate::SvelteCompiler;
