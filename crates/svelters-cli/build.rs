// Optional pack embedding: with SVELTERS_DOCS_EMBED_DIR=<dir> set at build time, every
// <dir>/*.svpack is compiled into the binary (include_bytes!), so a release `svelters` is one
// self-contained file. Without it the list is empty and the packs are found at run time
// (src/locate.rs). Build the packs first with `svelters docs-pack`.
use std::env;
use std::fmt::Write;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=SVELTERS_DOCS_EMBED_DIR");
    let mut src = String::from(
        "/// Packs compiled in at build time: (file name, bytes).\npub static EMBEDDED_PACKS: &[(&str, &[u8])] = &[\n",
    );
    if let Some(dir) = env::var_os("SVELTERS_DOCS_EMBED_DIR") {
        let dir = PathBuf::from(dir);
        println!("cargo:rerun-if-changed={}", dir.display());
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("SVELTERS_DOCS_EMBED_DIR={}: {e}", dir.display()))
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "svpack"))
            .collect();
        files.sort();
        assert!(
            !files.is_empty(),
            "SVELTERS_DOCS_EMBED_DIR={} has no *.svpack",
            dir.display()
        );
        for f in files {
            let f = f.canonicalize().unwrap();
            println!("cargo:rerun-if-changed={}", f.display());
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            writeln!(
                src,
                "    ({name:?}, include_bytes!({:?})),",
                f.display().to_string()
            )
            .unwrap();
        }
    }
    src.push_str("];\n");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("embedded_packs.rs");
    std::fs::write(out, src).unwrap();
}
