// Embeds counter-win.manifest (per-monitor DPI, maxversiontested) into the exe on MSVC targets.
// The GNU target and non-Windows hosts get nothing: the app still starts, without the manifest.
fn main() {
    println!("cargo:rerun-if-changed=counter-win.manifest");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if windows && msvc && std::env::var_os("SVELTERS_NO_MANIFEST").is_none() {
        let path = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("counter-win.manifest");
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}", path.display());
    }
}
