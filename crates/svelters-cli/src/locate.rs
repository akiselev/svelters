//! Finding the docs packs and the project's `svelters.toml`.
//!
//! Pack sources, first match wins per toolkit:
//! 1. `--pack <file|dir>` / `--ir <file|dir>` (repeatable), or `SVELTERS_DOCS_PACK` (`:`-separated
//!    files or directories). When any is given, nothing else is searched.
//! 2. Inside the svelters repo (a parent of the working directory has `docs/site/.generated/ir`):
//!    `docs/site/.generated/pack/<tk>.svpack`, rebuilt from the IR first when missing or older
//!    than the IR. If it cannot be written, the pack is built in memory.
//! 3. Compiled into the binary (`SVELTERS_DOCS_EMBED_DIR=<dir with *.svpack>` at build time).
//! 4. Installed next to the executable: `<exe dir>/<tk>.svpack`, `<exe dir>/docs/<tk>.svpack` or
//!    `<exe dir>/../share/svelters/docs/<tk>.svpack`.
//!
//! Nothing here touches the network or links a toolkit.

use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};

use crate::pack::{Pack, Source};

include!(concat!(env!("OUT_DIR"), "/embedded_packs.rs"));

/// Inputs that steer the search (filled from flags and the environment by the caller, and by
/// tests directly).
#[derive(Clone, Debug, Default)]
pub struct Search {
    pub cwd: PathBuf,
    /// Explicit pack files/directories (`--pack`, `SVELTERS_DOCS_PACK`).
    pub packs: Vec<PathBuf>,
    /// Explicit IR files/directories (`--ir`).
    pub irs: Vec<PathBuf>,
    /// Skip the repo, embedded and installed sources (tests).
    pub explicit_only: bool,
}

/// Loaded packs plus warnings for stderr.
pub struct Loaded {
    pub packs: Vec<Pack>,
    pub warnings: Vec<String>,
}

fn files_in(p: &Path, ext: &str) -> Vec<PathBuf> {
    if p.is_dir() {
        let mut v: Vec<PathBuf> = fs::read_dir(p)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|f| f.extension().is_some_and(|x| x == ext))
            .collect();
        v.sort();
        v
    } else {
        vec![p.to_path_buf()]
    }
}

fn push(out: &mut Loaded, pack: Result<Pack, String>, what: &Path) {
    match pack {
        Ok(p) => {
            if out.packs.iter().any(|q| q.toolkit() == p.toolkit()) {
                return; // earlier source wins
            }
            out.packs.push(p);
        }
        Err(e) => out.warnings.push(format!("{}: {e}", what.display())),
    }
}

/// The repo root when the working directory is inside the svelters repo with a generated IR.
pub fn repo_root(cwd: &Path) -> Option<PathBuf> {
    cwd.ancestors()
        .find(|d| d.join("docs/site/.generated/ir").is_dir())
        .map(Path::to_path_buf)
}

fn mtime(p: &Path) -> Option<std::time::SystemTime> {
    fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// Loads the repo pack for one IR file, rebuilding `docs/site/.generated/pack/<tk>.svpack` when
/// it is missing or older than the IR.
fn repo_pack(root: &Path, ir: &Path, warnings: &mut Vec<String>) -> Result<Pack, String> {
    let stem = ir.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
    let pack_path = root
        .join("docs/site/.generated/pack")
        .join(format!("{stem}.svpack"));
    let fresh = matches!((mtime(&pack_path), mtime(ir)), (Some(p), Some(i)) if p >= i);
    if fresh {
        let bytes = fs::read(&pack_path).map_err(|e| e.to_string())?;
        if let Ok(p) = Pack::open(
            Cow::Owned(bytes),
            Source::Repo(pack_path.display().to_string()),
        ) {
            return Ok(p);
        }
        // unreadable (older format): rebuild below
    }
    let ir_bytes = fs::read(ir).map_err(|e| e.to_string())?;
    let built = crate::pack::build(&ir_bytes)?;
    let src = match fs::create_dir_all(pack_path.parent().unwrap())
        .and_then(|_| fs::write(&pack_path, &built))
    {
        Ok(()) => Source::Repo(pack_path.display().to_string()),
        Err(e) => {
            warnings.push(format!(
                "could not write {}: {e}; using an in-memory pack",
                pack_path.display()
            ));
            Source::Ir(ir.display().to_string())
        }
    };
    Pack::open(Cow::Owned(built), src)
}

pub fn load(search: &Search) -> Loaded {
    let mut out = Loaded {
        packs: Vec::new(),
        warnings: Vec::new(),
    };
    let explicit = !search.packs.is_empty() || !search.irs.is_empty();
    for p in &search.packs {
        let p = search.cwd.join(p);
        for f in files_in(&p, "svpack") {
            let r = fs::read(&f)
                .map_err(|e| e.to_string())
                .and_then(|b| Pack::open(Cow::Owned(b), Source::File(f.display().to_string())));
            push(&mut out, r, &f);
        }
    }
    for p in &search.irs {
        let p = search.cwd.join(p);
        for f in files_in(&p, "json") {
            let r = fs::read(&f)
                .map_err(|e| e.to_string())
                .and_then(|b| Pack::from_ir(&b, Source::Ir(f.display().to_string())));
            push(&mut out, r, &f);
        }
    }
    if explicit || search.explicit_only {
        return out;
    }
    if let Some(root) = repo_root(&search.cwd) {
        for ir in files_in(&root.join("docs/site/.generated/ir"), "json") {
            let mut w = Vec::new();
            let r = repo_pack(&root, &ir, &mut w);
            out.warnings.extend(w);
            push(&mut out, r, &ir);
        }
    }
    for (name, bytes) in EMBEDDED_PACKS {
        push(
            &mut out,
            Pack::open(Cow::Borrowed(*bytes), Source::Embedded),
            Path::new(name),
        );
    }
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        for d in [
            dir.clone(),
            dir.join("docs"),
            dir.join("../share/svelters/docs"),
        ] {
            if !d.is_dir() {
                continue;
            }
            for f in files_in(&d, "svpack") {
                let r = fs::read(&f).map_err(|e| e.to_string()).and_then(|b| {
                    Pack::open(Cow::Owned(b), Source::Installed(f.display().to_string()))
                });
                push(&mut out, r, &f);
            }
        }
    }
    out
}

/// The app's toolkit from the nearest `svelters.toml` (`[app] toolkit = "…"`), with its path.
///
/// C0 reads only this key, with a line parser (no TOML dependency): a `toolkit` key inside the
/// `[app]` table, or `app.toolkit` at top level. C1 owns the full config and may switch to a TOML
/// crate.
pub fn project_toolkit(cwd: &Path) -> Option<(String, PathBuf)> {
    let path = cwd
        .ancestors()
        .map(|d| d.join("svelters.toml"))
        .find(|p| p.is_file())?;
    let text = fs::read_to_string(&path).ok()?;
    parse_toolkit(&text).map(|t| (t, path))
}

pub fn parse_toolkit(text: &str) -> Option<String> {
    let mut table = String::new();
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            table = line
                .trim_matches(|c| c == '[' || c == ']')
                .trim()
                .to_string();
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let k = k.trim().trim_matches('"');
        let full = if table.is_empty() {
            k.to_string()
        } else {
            format!("{table}.{k}")
        };
        if full == "app.toolkit" {
            let v = v.trim();
            let v = v
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| v.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))?;
            return Some(v.to_string());
        }
    }
    None
}

fn strip_comment(line: &str) -> &str {
    let mut in_str: Option<char> = None;
    for (i, c) in line.char_indices() {
        match (c, in_str) {
            ('"' | '\'', None) => in_str = Some(c),
            (q, Some(open)) if q == open => in_str = None,
            ('#', None) => return &line[..i],
            _ => {}
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolkit_from_app_table() {
        let t = "# app\n[app]\nname = \"x\" # n\ntoolkit = \"qt\"  # one per app\n[run]\ntoolkit = \"gtk\"\n";
        assert_eq!(parse_toolkit(t).as_deref(), Some("qt"));
    }

    #[test]
    fn toolkit_dotted_and_single_quotes() {
        assert_eq!(
            parse_toolkit("app.toolkit = 'gtk'\n").as_deref(),
            Some("gtk")
        );
        assert_eq!(parse_toolkit("[run]\ntoolkit = \"gtk\"\n"), None);
        assert_eq!(parse_toolkit("[app]\nname = \"a#b\"\n"), None);
    }
}
