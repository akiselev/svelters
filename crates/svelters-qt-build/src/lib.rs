//! Build-script helper for the Qt backend leaf crate (plans/architecture/backend-abi.md, "Qt build: leaf crate").
//!
//! ```no_run
//! // build.rs of a leaf crate
//! svelters_qt_build::Config::new()
//!     .qt_from_pkg_config("Qt6Widgets")
//!     .profile_dev_fat()
//!     .build();
//! ```
//!
//! For now the C++ is produced by `tools/metadata/qt/gen_cpp.py` into `OUT_DIR` (the generator moves
//! into this crate in M3). The result is compiled with the `cc` crate against Qt flags from
//! pkg-config, and link directives are emitted.
//!
//! `rerun-if-changed` is emitted only for the build config's inputs: `metadata/qt/**`,
//! `metadata/overrides/qt/**`, `metadata/inferred/qt/**` (the inferred layer, merged under the overrides),
//! the generator scripts/templates and the ABI header. Nothing from
//! the app or any dependent crate's Rust sources is listed, so editing those never reruns the
//! leaf crate's build script.

use std::env;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

const GENERATOR: &str = "tools/metadata/qt/gen_cpp.py";

/// Build configuration for the generated Qt backend.
#[derive(Clone, Debug)]
pub struct Config {
    qt_module: String,
    repo_root: Option<PathBuf>,
    metadata_dir: Option<PathBuf>,
    overrides_dir: Option<PathBuf>,
    inferred_dir: Option<PathBuf>,
    tags: Vec<String>,
    verification_tests: bool,
    fat: bool,
    dts_out: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl Config {
    pub fn new() -> Self {
        Config {
            qt_module: "Qt6Widgets".into(),
            repo_root: None,
            metadata_dir: None,
            overrides_dir: None,
            inferred_dir: None,
            tags: Vec::new(),
            verification_tests: false,
            fat: true,
            dts_out: None,
        }
    }

    /// pkg-config package that provides the Qt flags (default `Qt6Widgets`).
    pub fn qt_from_pkg_config(mut self, module: &str) -> Self {
        self.qt_module = module.to_string();
        self
    }

    /// Dev profile: every annotated element/method is compiled in. (The only profile in P.)
    pub fn profile_dev_fat(mut self) -> Self {
        self.fat = true;
        self
    }

    /// Repository root holding `tools/`, `metadata/` (default: found by walking up from the manifest dir).
    pub fn repo_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.repo_root = Some(root.into());
        self
    }

    pub fn metadata_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.metadata_dir = Some(dir.into());
        self
    }

    pub fn overrides_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.overrides_dir = Some(dir.into());
        self
    }

    /// The inferred annotation layer (default `metadata/inferred/qt`; tools/metadata/qt/infer.py).
    pub fn inferred_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.inferred_dir = Some(dir.into());
        self
    }

    /// Restrict the backend to these element tags (default: every annotated element).
    pub fn elements<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    /// Also compile the generated verification tests into a separate static library
    /// (`svl_qt_verify`); it is only linked into binaries that reference `svl_qt_verify_run`.
    pub fn with_verification_tests(mut self) -> Self {
        self.verification_tests = true;
        self
    }

    /// Also write the element `.d.ts` to a stable path (for example in a checked-in `types/`
    /// directory). The file is only rewritten when its content changes.
    pub fn dts_out(mut self, path: impl Into<PathBuf>) -> Self {
        self.dts_out = Some(path.into());
        self
    }

    /// Run the generator, compile, and print cargo directives. Panics with a message on failure.
    pub fn build(self) {
        if let Err(e) = self.try_build() {
            panic!("svelters-qt-build: {e}");
        }
    }

    pub fn try_build(self) -> Result<(), String> {
        let manifest = PathBuf::from(env_var("CARGO_MANIFEST_DIR")?);
        let out_dir = PathBuf::from(env_var("OUT_DIR")?);
        let root = match &self.repo_root {
            Some(r) => r.clone(),
            None => find_root(&manifest)?,
        };
        let metadata = self.metadata_dir.clone().unwrap_or_else(|| root.join("metadata/qt"));
        let overrides = self.overrides_dir.clone().unwrap_or_else(|| root.join("metadata/overrides/qt"));
        let inferred = self.inferred_dir.clone().unwrap_or_else(|| root.join("metadata/inferred/qt"));
        let abi_include = root.join("crates/svelters-abi/include");

        // rerun-if-changed: config inputs only.
        for p in [
            &metadata,
            &overrides,
            &inferred,
            &root.join("tools/metadata/qt"),
            &root.join("tools/metadata/common/svmeta.py"),
            &abi_include,
        ] {
            println!("cargo:rerun-if-changed={}", p.display());
        }
        println!("cargo:rerun-if-env-changed=SVELTERS_PYTHON");
        println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");

        // 1. generate
        let gen_dir = out_dir.join("svl_qt");
        std::fs::create_dir_all(&gen_dir).map_err(|e| e.to_string())?;
        let mut toml = String::new();
        let q = |p: &Path| format!("{:?}", p.to_string_lossy());
        writeln!(toml, "[input]\nmetadata = {}\noverrides = {}\ninferred = {}\n", q(&metadata), q(&overrides), q(&inferred)).unwrap();
        writeln!(toml, "[output]\nbackend = \"svl_qt_backend.cpp\"\nverify = \"svl_qt_verify.cpp\"\ndts = \"qt.d.ts\"\n").unwrap();
        let tags: Vec<String> = self.tags.iter().map(|t| format!("{t:?}")).collect();
        writeln!(toml, "[elements]\ntags = [{}]", tags.join(", ")).unwrap();
        let cfg_path = out_dir.join("svl_qt_gen.toml");
        std::fs::write(&cfg_path, toml).map_err(|e| e.to_string())?;

        let python = env::var("SVELTERS_PYTHON").unwrap_or_else(|_| "python3".into());
        let mut gen_cmd = Command::new(&python);
        gen_cmd.arg("-I").arg(root.join(GENERATOR)).arg("--config").arg(&cfg_path).arg("--out").arg(&gen_dir);
        if let Some(dts) = &self.dts_out {
            gen_cmd.arg("--dts").arg(root.join(dts));
        }
        let status = gen_cmd
            .status()
            .map_err(|e| format!("cannot run {python}: {e}"))?;
        if !status.success() {
            return Err(format!("{GENERATOR} failed ({status})"));
        }

        // 2. Qt flags
        let cflags = pkg_config(&self.qt_module, "--cflags")?;
        let libs = pkg_config(&self.qt_module, "--libs")?;

        // 3. compile. Verification library first so that link order puts it before the backend it uses.
        let mut sources = vec![];
        if self.verification_tests {
            sources.push(("svl_qt_verify", "svl_qt_verify.cpp"));
        }
        sources.push(("svl_qt_backend", "svl_qt_backend.cpp"));
        for (lib, file) in sources {
            let mut b = cc::Build::new();
            b.cpp(true).std("c++17").pic(true).warnings(false).include(&abi_include);
            apply_cflags(&mut b, &cflags);
            b.file(gen_dir.join(file));
            b.compile(lib);
        }

        // 4. link Qt
        let toks: Vec<&str> = libs.split_whitespace().collect();
        for t in &toks {
            if let Some(d) = t.strip_prefix("-L") {
                println!("cargo:rustc-link-search=native={d}");
            }
        }
        for t in &toks {
            if let Some(l) = t.strip_prefix("-l") {
                println!("cargo:rustc-link-lib={l}");
            }
        }
        println!("cargo:rustc-env=SVL_QT_GENERATED_DIR={}", gen_dir.display());
        let _ = self.fat;
        Ok(())
    }
}

fn env_var(k: &str) -> Result<String, String> {
    env::var(k).map_err(|_| format!("{k} not set (use from build.rs)"))
}

fn find_root(from: &Path) -> Result<PathBuf, String> {
    from.ancestors()
        .find(|d| d.join(GENERATOR).exists())
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("cannot find {GENERATOR} above {}; set Config::repo_root", from.display()))
}

fn pkg_config(module: &str, flag: &str) -> Result<String, String> {
    let out = Command::new("pkg-config")
        .arg(flag)
        .arg(module)
        .output()
        .map_err(|e| format!("cannot run pkg-config: {e}"))?;
    if !out.status.success() {
        return Err(format!("pkg-config {flag} {module} failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn apply_cflags(b: &mut cc::Build, cflags: &str) {
    let mut it = cflags.split_whitespace();
    while let Some(t) = it.next() {
        if let Some(d) = t.strip_prefix("-I") {
            b.include(d);
        } else if t == "-isystem" {
            if let Some(d) = it.next() {
                b.include(d);
            }
        } else if let Some(d) = t.strip_prefix("-D") {
            match d.split_once('=') {
                Some((k, v)) => b.define(k, v),
                None => b.define(d, None),
            };
        }
    }
}
