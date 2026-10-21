//! The non-GUI type prober (N4): lists the declared Objective-C properties of Foundation/AppKit classes
//! from the runtime, which objc2's generated sources do not mark (a read-only `@property` is an ordinary
//! zero-argument method there). `tools/metadata/appkit/types_appkit_classify.py` reads the committed
//! output (`metadata/probe/appkit/types-<SDK>.json`) and turns declared read-only properties into getters.
//!
//!   types_probe CLASSES_FILE > types.json      (one class name per line; macOS only)
//!
//! Only names and attribute strings are written (no values). The macos-appkit CI job runs it and uploads
//! the output with the probe artifact.

#[cfg(target_os = "macos")]
mod mac {
    use objc2::runtime::AnyClass;
    use serde_json::{Value as J, json};
    use std::ffi::{CStr, c_uint, c_void};

    unsafe extern "C" {
        fn free(p: *mut c_void);
    }

    fn props_of(cls: &AnyClass, class_prop: bool) -> Vec<J> {
        let mut out = Vec::new();
        let mut n: c_uint = 0;
        let list = unsafe { objc2::ffi::class_copyPropertyList(cls, &mut n) };
        if list.is_null() {
            return out;
        }
        for i in 0..n as usize {
            let p = unsafe { *list.add(i) };
            let name = unsafe { CStr::from_ptr(objc2::ffi::property_getName(p)) }.to_string_lossy().into_owned();
            let attrs = unsafe { CStr::from_ptr(objc2::ffi::property_getAttributes(p)) }.to_string_lossy().into_owned();
            let parts: Vec<&str> = attrs.split(',').collect();
            let readonly = parts.contains(&"R");
            let getter = parts.iter().find_map(|a| a.strip_prefix('G')).map(str::to_string);
            out.push(json!({ "name": name, "attrs": attrs, "readonly": readonly, "getter": getter, "class_prop": class_prop }));
        }
        unsafe { free(list as *mut c_void) };
        out.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        out
    }

    pub fn main() {
        let path = std::env::args().nth(1).expect("usage: types_probe CLASSES_FILE");
        // Load AppKit (NSApplication and the backend) so its classes are registered with the runtime.
        let _ = svelters_appkit::testing::Harness::shared();
        let names = std::fs::read_to_string(&path).expect("read the class list");
        let mut classes = serde_json::Map::new();
        let mut missing = Vec::new();
        for name in names.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let Some(cls) = svelters_apple::objc::bridge::class_named(name) else {
                missing.push(J::from(name));
                continue;
            };
            let mut props = props_of(cls, false);
            props.extend(props_of(cls.metaclass(), true));
            classes.insert(name.to_string(), json!({ "props": props }));
        }
        let os = std::process::Command::new("sw_vers").arg("-productVersion").output().ok();
        let macos = os.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
        let doc = json!({ "schema": "svelters-appkit-types-probe/v1", "runtime": { "macos": macos }, "missing": missing, "classes": classes });
        println!("{}", serde_json::to_string_pretty(&doc).unwrap());
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    mac::main();
    #[cfg(not(target_os = "macos"))]
    eprintln!("types_probe: macOS only (the macos-appkit CI job runs it)");
}
