//! AppKit widget gallery tests: one case per page of `fixtures/gallery-appkit` (the AppKit twin of
//! `examples/counter/tests/gallery.rs`).
//!
//! Needs the built bundle (`pnpm -r build`). Each case loads the gallery in a fresh host and backend,
//! clicks the page's sidebar button natively (`performClick:` on the real NSButton), then checks:
//! - no JS errors;
//! - the page's native classes are live (every handle the backend holds, so non-views such as
//!   NSMenu, NSMenuItem and NSTabViewItem count);
//! - every `echo <name>: N` label on the page reads 0 (the app's own writes did not come back as
//!   change events: echo suppression for each two-way binding on the page);
//! - a screenshot of the window lands in `target/gallery/appkit-<page>.png` (`screencapture -l`).
//!
//! Some pages then drive the page natively and check the result (a menu item's action, a tab
//! selection through the delegate).
//!
//! Adding a page: put `NN-family.svelte` under `fixtures/gallery-appkit/src/pages/` and add one
//! `gallery_page!` line and its case in `main` below (fixtures/gallery-common/check-pages.mjs
//! enforces the line). An empty suite on other platforms.

fn main() {
    #[cfg(target_os = "macos")]
    svelters_apple::testing::run_cases(
        true,
        &[
            ("every_page_has_a_case", mac::every_page_has_a_case),
            ("appkit_00_overview", mac::appkit_00_overview),
            ("appkit_01_buttons", mac::appkit_01_buttons),
            ("appkit_02_text", mac::appkit_02_text),
            ("appkit_03_values", mac::appkit_03_values),
            ("appkit_04_containers", mac::appkit_04_containers),
            ("appkit_05_menus", mac::appkit_05_menus),
        ],
    );
    #[cfg(not(target_os = "macos"))]
    svelters_apple::testing::run_cases(false, &[]);
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Sel};
    use objc2_foundation::NSString;
    use std::path::{Path, PathBuf};
    use svelters_abi::BackendRef;
    use svelters_appkit::AppKitBackend;
    use svelters_appkit::testing::pump;
    use svelters_host::{Host, HostConfig};

    /// The page names `every_page_has_a_case` compares with the pages directory.
    const CASES: &[&str] = &["00-overview", "01-buttons", "02-text", "03-values", "04-containers", "05-menus"];

    macro_rules! gallery_page {
        ($name:ident, "appkit", $page:literal, [$($class:literal),* $(,)?]) => {
            pub fn $name() {
                let g = Gallery::open($page, &[$($class),*]);
                g.close();
            }
        };
        ($name:ident, "appkit", $page:literal, [$($class:literal),* $(,)?], $then:expr) => {
            pub fn $name() {
                let g = Gallery::open($page, &[$($class),*]);
                ($then)(&g);
                g.close();
            }
        };
    }

    gallery_page!(appkit_00_overview, "appkit", "00-overview", ["NSWindow", "NSStackView", "NSTextField", "NSButton", "NSSecureTextField", "NSSlider", "NSProgressIndicator", "NSBox"]);
    gallery_page!(appkit_01_buttons, "appkit", "01-buttons", ["NSButton", "NSSwitch", "NSSegmentedControl", "NSPopUpButton", "NSStepper"], |g: &Gallery| {
        // The $effect writes reached the native controls (and, checked by `open`, sent no change events).
        assert!(g.has_label("switch on"), "bind:state wrote the switch: {:?}", g.labels());
        assert!(g.has_label("segment 2"), "bind:selected-segment: {:?}", g.labels());
        assert!(g.has_label("picked 1"), "bind:index-of-selected-item: {:?}", g.labels());
        assert!(g.has_label("steps 4"), "bind:double-value on the stepper: {:?}", g.labels());
        let seg = g.first("NSSegmentedControl");
        let sel: isize = unsafe { msg_send![&*seg, selectedSegment] };
        assert_eq!(sel, 2, "the segmented control shows the bound segment");
        let n: isize = unsafe { msg_send![&*seg, segmentCount] };
        assert_eq!(n, 3, "labels made three segments");
    });
    gallery_page!(appkit_02_text, "appkit", "02-text", ["NSTextField", "NSSearchField", "NSTokenField", "NSComboBox", "NSTextView", "NSScrollView"], |g: &Gallery| {
        assert!(g.has_label("hello Ada"), "bind:string-value: {:?}", g.labels());
        assert!(g.has_label("flavor 1"), "bind:index-of-selected-item on the combo box: {:?}", g.labels());
        let tv = g.first("NSTextView");
        let s: Retained<NSString> = unsafe { msg_send![&*tv, string] };
        assert!(s.to_string().starts_with("A text view"), "bind:string wrote the text view: {s}");
        let sv = g.first("NSScrollView");
        let doc: Option<Retained<AnyObject>> = unsafe { msg_send![&*sv, documentView] };
        assert!(doc.is_some_and(|d| class_name(&d) == "NSTextView"), "the text view is the scroll view's document view");
    });
    gallery_page!(appkit_03_values, "appkit", "03-values", ["NSSlider", "NSProgressIndicator", "NSLevelIndicator", "NSDatePicker", "NSColorWell", "NSPathControl", "NSImageView"], |g: &Gallery| {
        assert!(g.has_label("level 60"), "bind:double-value on the slider: {:?}", g.labels());
        let pc = g.first("NSPathControl");
        let url: Option<Retained<AnyObject>> = unsafe { msg_send![&*pc, URL] };
        let path: Option<Retained<NSString>> = url.and_then(|u| unsafe { msg_send![&*u, path] });
        assert_eq!(path.map(|p| p.to_string()).as_deref(), Some("/Applications/Utilities"), "url from a path string");
        let iv = g.first("NSImageView");
        let img: Option<Retained<AnyObject>> = unsafe { msg_send![&*iv, image] };
        assert!(img.is_some(), "image from an SF Symbol name");
    });
    gallery_page!(appkit_04_containers, "appkit", "04-containers", ["NSTabView", "NSTabViewItem", "NSSplitView", "NSBox", "NSVisualEffectView", "NSView"], |g: &Gallery| {
        let tabs = g.first("NSTabView");
        let n: isize = unsafe { msg_send![&*tabs, numberOfTabViewItems] };
        assert_eq!(n, 2, "two tab-view-item children");
        let split = g.first("NSSplitView");
        let panes: Retained<AnyObject> = unsafe { msg_send![&*split, arrangedSubviews] };
        let np: usize = unsafe { msg_send![&*panes, count] };
        assert_eq!(np, 2, "two split panes");
        // A native selection goes through the backend's delegate to JS: should-select, then did-select.
        let (sel0, asked0) = g.tab_counts();
        let _: () = unsafe { msg_send![&*tabs, selectTabViewItemAtIndex: 1isize] };
        pump(50);
        let (sel1, asked1) = g.tab_counts();
        assert_eq!((sel1 - sel0, asked1 - asked0), (1, 1), "one asked and one did-select per native selection: {:?}", g.labels());
        let item: Retained<AnyObject> = unsafe { msg_send![&*tabs, selectedTabViewItem] };
        let label: Retained<NSString> = unsafe { msg_send![&*item, label] };
        assert_eq!(label.to_string(), "Advanced");
        g.no_errors();
    });
    gallery_page!(appkit_05_menus, "appkit", "05-menus", ["NSMenu", "NSMenuItem", "NSPopUpButton", "NSView"], |g: &Gallery| {
        let popup = g.first("NSPopUpButton");
        let titles: Retained<AnyObject> = unsafe { msg_send![&*popup, itemTitles] };
        let n: usize = unsafe { msg_send![&*titles, count] };
        assert_eq!(n, 4, "the pop-up's items are the menu-item children");
        // A menu item's target-action, sent the way AppKit sends it when the user picks the item.
        let item = g.find("NSMenuItem", |o| {
            let t: Retained<NSString> = unsafe { msg_send![o, title] };
            t.to_string() == "Banana"
        });
        let action: Option<Sel> = unsafe { msg_send![&*item, action] };
        let target: Option<Retained<AnyObject>> = unsafe { msg_send![&*item, target] };
        let app: Retained<AnyObject> = unsafe { msg_send![objc2::class!(NSApplication), sharedApplication] };
        let sent: bool = unsafe { msg_send![&*app, sendAction: action.expect("onaction installed an action"), to: target.as_deref(), from: &*item] };
        assert!(sent, "NSApp delivered the menu item's action");
        pump(50);
        assert!(g.has_label("picked: Banana"), "the menu item's onaction ran: {:?}", g.labels());
        // The context menu sits in the plain view's `menu` slot.
        let view = g.find("NSView", |o| {
            let m: Option<Retained<AnyObject>> = unsafe { msg_send![o, menu] };
            m.is_some()
        });
        let menu: Retained<AnyObject> = unsafe { msg_send![&*view, menu] };
        let items: isize = unsafe { msg_send![&*menu, numberOfItems] };
        assert_eq!(items, 2, "the context menu has its two items");
        g.no_errors();
    });

    pub fn every_page_has_a_case() {
        let mut pages = pages();
        pages.sort();
        assert_eq!(pages, CASES, "pages in fixtures/gallery-appkit/src/pages vs CASES in this file");
    }

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn pages() -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(root().join("fixtures/gallery-appkit/src/pages"))
            .expect("pages dir")
            .filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".svelte").map(str::to_string))
            .collect();
        v.sort();
        v
    }

    fn class_name(o: &AnyObject) -> String {
        o.class().name().to_string_lossy().into_owned()
    }

    fn is_kind(o: &AnyObject, class: &str) -> bool {
        let Some(cls) = objc2::runtime::AnyClass::get(&std::ffi::CString::new(class).unwrap()) else { return false };
        unsafe { msg_send![o, isKindOfClass: cls] }
    }

    struct Gallery {
        page: &'static str,
        backend: AppKitBackend,
        host: Host<'static>,
        win: Retained<AnyObject>,
    }

    impl Gallery {
        /// Loads the gallery, clicks the page's sidebar button and runs the common checks.
        fn open(page: &'static str, classes: &[&str]) -> Gallery {
            let bundle = root().join("fixtures/gallery-appkit/dist/app.js");
            assert!(bundle.is_file(), "{} is missing: run `pnpm install && pnpm -r build` first", bundle.display());
            let idx = pages().iter().position(|p| p == page).unwrap_or_else(|| panic!("no page {page} in gallery-appkit"));
            let backend = AppKitBackend::new().unwrap();
            let vt = backend.vtable();
            let bref = unsafe { BackendRef::from_raw(vt.as_raw()) }.unwrap();
            let host = Host::new(bref, HostConfig::default()).unwrap();
            backend.attach_host(host.host_callbacks());
            host.load_bundle(&bundle).unwrap();
            pump(100);
            let wins = backend.toplevels();
            let win = backend.object(*wins.last().expect("a window")).unwrap();
            let g = Gallery { page, backend, host, win };
            g.no_errors();
            // window > outer stack-view > sidebar stack-view > button per page (App.svelte keeps the sidebar first).
            let outer: Retained<AnyObject> = unsafe { msg_send![&*g.win, contentView] };
            let sidebar = arranged(&outer)[0].clone();
            let buttons = arranged(&sidebar);
            assert_eq!(buttons.len(), pages().len(), "one sidebar button per page");
            let _: () = unsafe { msg_send![&*buttons[idx], performClick: Option::<&AnyObject>::None] };
            pump(150);
            g.no_errors();
            let have = g.live_classes();
            for c in classes {
                assert!(g.objects().iter().any(|o| is_kind(o, c)), "page {page}: no live {c}: {have:?}");
            }
            for l in g.labels() {
                if let Some(rest) = l.strip_prefix("echo ") {
                    assert!(rest.ends_with(": 0"), "page {page}: '{l}': a programmatic write came back as a change event");
                }
            }
            g.screenshot();
            g
        }

        fn no_errors(&self) {
            let errors = self.host.take_errors();
            assert!(errors.is_empty(), "page {}: JS errors: {errors:?}", self.page);
        }

        fn objects(&self) -> Vec<Retained<AnyObject>> {
            self.backend.handles().into_iter().filter_map(|h| self.backend.object(h)).collect()
        }

        fn live_classes(&self) -> Vec<String> {
            let mut v: Vec<String> = self.objects().iter().map(|o| class_name(o)).collect();
            v.sort();
            v.dedup();
            v
        }

        fn find(&self, class: &str, pred: impl Fn(&AnyObject) -> bool) -> Retained<AnyObject> {
            self.objects()
                .into_iter()
                .find(|o| is_kind(o, class) && pred(o))
                .unwrap_or_else(|| panic!("page {}: no matching {class}", self.page))
        }

        fn first(&self, class: &str) -> Retained<AnyObject> {
            self.find(class, |o| class_name(o) == class)
        }

        /// The string values of every live NSTextField (labels and fields).
        fn labels(&self) -> Vec<String> {
            self.objects()
                .iter()
                .filter(|o| class_name(o) == "NSTextField")
                .map(|o| {
                    let s: Retained<NSString> = unsafe { msg_send![&**o, stringValue] };
                    s.to_string()
                })
                .collect()
        }

        fn has_label(&self, text: &str) -> bool {
            self.labels().iter().any(|l| l == text)
        }

        /// (selections, asked) from the containers page's `selections N, asked M` label.
        fn tab_counts(&self) -> (i64, i64) {
            let l = self.labels().into_iter().find(|l| l.starts_with("selections ")).expect("the selections label");
            let nums: Vec<i64> = l.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).map(|s| s.parse().unwrap()).collect();
            (nums[0], nums[1])
        }

        fn screenshot(&self) {
            let dir = root().join("target/gallery");
            std::fs::create_dir_all(&dir).unwrap();
            let png = dir.join(format!("appkit-{}.png", self.page));
            let _ = std::fs::remove_file(&png);
            let n: isize = unsafe { msg_send![&*self.win, windowNumber] };
            // screencapture asks the window server for the window's backing store; the main thread keeps
            // pumping meanwhile.
            let p = png.clone();
            let t = std::thread::spawn(move || {
                std::process::Command::new("screencapture").args(["-x", "-o", &format!("-l{n}")]).arg(&p).status().is_ok_and(|s| s.success())
            });
            while !t.is_finished() {
                pump(20);
            }
            let ok = t.join().unwrap();
            let size = std::fs::metadata(&png).map(|m| m.len()).unwrap_or(0);
            assert!(ok && size > 0, "page {}: no screenshot at {} (Screen Recording permission?)", self.page, png.display());
        }

        fn close(self) {
            self.no_errors();
            let _: () = unsafe { msg_send![&*self.win, orderOut: Option::<&AnyObject>::None] };
            pump(20);
        }
    }

    fn arranged(stack: &AnyObject) -> Vec<Retained<AnyObject>> {
        let items: Retained<AnyObject> = unsafe { msg_send![stack, arrangedSubviews] };
        let n: usize = unsafe { msg_send![&*items, count] };
        (0..n).map(|i| unsafe { msg_send![&*items, objectAtIndex: i] }).collect()
    }
}
