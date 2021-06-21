pub mod compiler;
pub mod task;
pub mod typescript;

pub(in crate) mod dependencies;
pub(in crate) mod inner;
mod loader;
mod resolver;

use inner::*;

pub use compiler::*;

pub async fn compile<P: AsRef<Path>>(compiler: &SvelteCompiler, path: P) -> Result<String> {
    let path_str = path.as_ref().to_str().unwrap();
    tracing::info!("Compiling...");
    if path_str.ends_with(".ts") {
        let path_mapping = FilePathMapping::empty();
        let cm = Arc::new(SourceMap::with_file_loader(
            Box::new(FsLoader),
            path_mapping,
        ));
        let input = cm.load_file(path.as_ref()).map_err(Error::from)?;
        transpile_typescript(&cm, path_str, (&*input).into())
    } else if path_str.ends_with(".svelte") {
        let source = std::fs::read_to_string(path.as_ref())?;
        compiler.compile(path_str, source).await
    } else {
        let source = std::fs::read_to_string(path.as_ref())?;
        Ok(source)
    }
}

pub fn compile_sync<P: AsRef<Path>>(compiler: &SvelteCompiler, path: P) -> Result<String> {
    let path_str = path.as_ref().to_str().unwrap();
    tracing::info!("Compiling...");
    if path_str.ends_with(".ts") {
        let path_mapping = FilePathMapping::empty();
        let cm = Arc::new(SourceMap::with_file_loader(
            Box::new(FsLoader),
            path_mapping,
        ));
        let input = cm.load_file(path.as_ref()).map_err(Error::from)?;
        transpile_typescript(&cm, path_str, (&*input).into())
    } else if path_str.ends_with(".svelte") {
        let source = std::fs::read_to_string(path.as_ref())?;
        compiler.compile_sync(path_str, source)
    } else {
        let source = std::fs::read_to_string(path.as_ref())?;
        Ok(source)
    }
}
