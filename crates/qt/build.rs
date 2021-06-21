use semver::Version;

fn main() {
    eprintln!("cargo:warning={:?}", std::env::vars().collect::<Vec<_>>());

    let qt_include_path = std::env::var("DEP_QT_INCLUDE_PATH").unwrap();
    let qt_library_path = std::env::var("DEP_QT_LIBRARY_PATH").unwrap();
    let qt_version = std::env::var("DEP_QT_VERSION")
        .unwrap()
        .parse::<Version>()
        .expect("Parsing Qt version failed");

    let mut config = cpp_build::Config::new();
    if cfg!(target_os = "macos") {
        config.flag("-F");
        config.flag(qt_library_path.trim());
    }
    if qt_version >= Version::new(6, 0, 0) {
        config.flag_if_supported("-std=c++17");
        config.flag_if_supported("/std:c++17");
    }
    config
        .include(qt_include_path.trim())
        .include(std::fs::canonicalize(format!("./src")).unwrap().as_os_str())
        .build("src/lib.rs");

    for minor in 7..=15 {
        if qt_version >= Version::new(5, minor, 0) {
            println!("cargo:rustc-cfg=qt_{}_{}", 5, minor);
        }
    }
    let mut minor = 0;
    while qt_version >= Version::new(6, minor, 0) {
        println!("cargo:rustc-cfg=qt_{}_{}", 6, minor);
        minor += 1;
    }
}

// fn main() {
//     let qt_include_path = std::env::var("DEP_QT_INCLUDE_PATH").unwrap();
//     let qt_library_path = std::env::var("DEP_QT_LIBRARY_PATH").unwrap();

//     #[allow(unused_mut)]
//     let mut config = cpp_build::Config::new();

//     if cfg!(target_os = "macos") {
//         config.flag("-F");
//         config.flag(&qt_library_path);
//     }

//     config
//         .include(&qt_include_path)
//         .include(std::fs::canonicalize(format!("./src")).unwrap().as_os_str())
//         .include(format!("{}/QtGui", qt_include_path))
//         .include(format!("{}/QtCore", qt_include_path))
//         .build("src/lib.rs");

//     let macos_lib_search = if cfg!(target_os = "macos") {
//         "=framework"
//     } else {
//         ""
//     };
//     let macos_lib_framework = if cfg!(target_os = "macos") { "" } else { "5" };

//     let debug = std::env::var("DEBUG").ok().map_or(false, |s| s == "true");
//     let windows_dbg_suffix = if debug {
//         if cfg!(target_os = "windows") {
//             println!("cargo:rustc-link-lib=msvcrtd");
//             "d"
//         } else {
//             ""
//         }
//     } else {
//         ""
//     };
//     println!(
//         "cargo:rustc-link-search{}={}",
//         macos_lib_search,
//         qt_library_path.trim()
//     );
//     println!(
//         "cargo:rustc-link-lib{}=Qt{}Core{}",
//         macos_lib_search, macos_lib_framework, windows_dbg_suffix
//     );
//     println!(
//         "cargo:rustc-link-lib{}=Qt{}Gui{}",
//         macos_lib_search, macos_lib_framework, windows_dbg_suffix
//     );
//     println!(
//         "cargo:rustc-link-lib{}=Qt{}Widgets{}",
//         macos_lib_search, macos_lib_framework, windows_dbg_suffix
//     );
// }
