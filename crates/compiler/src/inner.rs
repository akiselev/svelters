pub(in crate) use std::{
    path::Path,
    sync::mpsc::{self, Receiver, SyncSender},
    sync::Arc,
};

pub(in crate) use node_resolve::Resolver as NodeResolver;
pub(in crate) use parking_lot::RwLock;
pub(in crate) use rquickjs::{
    bind, BuiltinLoader, BuiltinResolver, Context, Ctx, Error, FileResolver, FromJs, Function,
    IntoJs, NativeLoader, Result, Runtime, ScriptLoader, Value,
};
pub(in crate) use swc_common::{
    errors::{ColorConfig, Handler},
    input::StringInput,
    BytePos, FileLoader, FilePathMapping, SourceMap,
};
pub(in crate) use swc_ecma_codegen::{text_writer::JsWriter, Config, Emitter};
pub(in crate) use swc_ecma_parser::{lexer::Lexer, Parser as JsParser, Syntax, TsConfig};
pub(in crate) use swc_ecma_visit::FoldWith;
