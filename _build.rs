fn main() {
    // npm_rs::NpmEnv::default()
    //     .with_env("NODE_ENV", "development")
    //     .init()
    //     .install(Some(&["svelte"]))
    //     .exec()
    //     .unwrap();

    // let qt_include_path = std::env::var("DEP_QT_INCLUDE_PATH").unwrap();
    // let qt_library_path = std::env::var("DEP_QT_LIBRARY_PATH").unwrap();

    // #[allow(unused_mut)]
    // let mut config = cpp_build::Config::new();

    // if cfg!(target_os = "macos") {
    //     config.flag("-F");
    //     config.flag(&qt_library_path);
    // }

    // config
    //     .include(&qt_include_path)
    //     .include(std::fs::canonicalize(format!("./src")).unwrap().as_os_str())
    //     // .include(format!("{}/QtQuick", qt_include_path))
    //     .include(format!("{}/QtGui", qt_include_path))
    //     .include(format!("{}/QtCore", qt_include_path))
    //     .build("src/lib.rs");

    // let macos_lib_search = if cfg!(target_os = "macos") {
    //     "=framework"
    // } else {
    //     ""
    // };
    // let macos_lib_framework = if cfg!(target_os = "macos") { "" } else { "5" };

    // let debug = std::env::var("DEBUG").ok().map_or(false, |s| s == "true");
    // let windows_dbg_suffix = if debug {
    //     if cfg!(target_os = "windows") {
    //         println!("cargo:rustc-link-lib=msvcrtd");
    //         "d"
    //     } else {
    //         ""
    //     }
    // } else {
    //     ""
    // };
    // println!(
    //     "cargo:rustc-link-search{}={}",
    //     macos_lib_search,
    //     qt_library_path.trim()
    // );
    // println!(
    //     "cargo:rustc-link-lib{}=Qt{}Widgets{}",
    //     macos_lib_search, macos_lib_framework, windows_dbg_suffix
    // );
    // println!(
    //     "cargo:rustc-link-lib{}=Qt{}Gui{}",
    //     macos_lib_search, macos_lib_framework, windows_dbg_suffix
    // );
    // println!(
    //     "cargo:rustc-link-lib{}=Qt{}Core{}",
    //     macos_lib_search, macos_lib_framework, windows_dbg_suffix
    // );
}
