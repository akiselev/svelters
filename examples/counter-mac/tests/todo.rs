//! The styled todo app (`fixtures/todo-mac`) on AppKit: ST2's decl dialect end to end.
//!
//! Needs the built bundle (`pnpm -r build`). Each case loads the app in a fresh host and backend,
//! drives it natively (performClick:, text change notifications, the tracking area's owner) and checks
//! what the stylesheet did to the native objects: fonts, layer backgrounds, borders, radius, shadow,
//! attributed text, alpha for :disabled, the :hover block, the colour scheme and the scroll view.
//! Screenshots land in `target/todo/`. An empty suite on other platforms.

fn main() {
    #[cfg(target_os = "macos")]
    svelters_apple::testing::run_cases(
        true,
        &[
            ("styles_applied", mac::styles_applied),
            ("add_toggle_delete", mac::add_toggle_delete),
            ("hover_block", mac::hover_block),
            ("scheme_switch", mac::scheme_switch),
            ("list_scrolls", mac::list_scrolls),
        ],
    );
    #[cfg(not(target_os = "macos"))]
    svelters_apple::testing::run_cases(false, &[]);
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use objc2_foundation::{NSRect, NSString};
    use std::path::{Path, PathBuf};
    use svelters_abi::BackendRef;
    use svelters_appkit::AppKitBackend;
    use svelters_appkit::testing::pump;
    use svelters_host::{Host, HostConfig};

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn frame(v: &AnyObject) -> NSRect {
        unsafe { msg_send![v, frame] }
    }

    fn class_name(o: &AnyObject) -> String {
        o.class().name().to_string_lossy().into_owned()
    }

    fn string_of(o: &AnyObject, sel: &str) -> String {
        use objc2::runtime::MessageReceiver;
        let sel = objc2::runtime::Sel::register(&std::ffi::CString::new(sel).unwrap());
        let p: *mut AnyObject = unsafe { o.send_message(sel, ()) };
        let s: Option<Retained<NSString>> = unsafe { Retained::retain(p as *mut NSString) };
        s.map(|s| s.to_string()).unwrap_or_default()
    }

    /// sRGB components of an NSColor (None for nil).
    fn rgba(c: Option<&AnyObject>) -> Option<[f64; 4]> {
        let c = c?;
        let space: Retained<AnyObject> = unsafe { msg_send![class!(NSColorSpace), sRGBColorSpace] };
        let c: Option<Retained<AnyObject>> = unsafe { msg_send![c, colorUsingColorSpace: &*space] };
        let c = c?;
        unsafe { Some([msg_send![&*c, redComponent], msg_send![&*c, greenComponent], msg_send![&*c, blueComponent], msg_send![&*c, alphaComponent]]) }
    }

    fn hex(c: [f64; 4]) -> String {
        format!("#{:02x}{:02x}{:02x}", (c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8)
    }

    /// The layer's background as an NSColor-free hex (CGColor components through NSColor).
    #[repr(transparent)]
    #[derive(Clone, Copy)]
    struct CGColorRef(*const std::ffi::c_void);
    // SAFETY: a CGColorRef is a pointer to the opaque `struct CGColor`.
    unsafe impl objc2::Encode for CGColorRef {
        const ENCODING: objc2::Encoding = objc2::Encoding::Pointer(&objc2::Encoding::Struct("CGColor", &[]));
    }

    fn layer_bg(v: &AnyObject) -> Option<String> {
        let layer: Option<Retained<AnyObject>> = unsafe { msg_send![v, layer] };
        let layer = layer?;
        let cg: CGColorRef = unsafe { msg_send![&*layer, backgroundColor] };
        if cg.0.is_null() {
            return None;
        }
        let c: Option<Retained<AnyObject>> = unsafe { msg_send![class!(NSColor), colorWithCGColor: cg] };
        rgba(c.as_deref()).map(hex)
    }

    fn layer_f64(v: &AnyObject, key: &str) -> f64 {
        let layer: Option<Retained<AnyObject>> = unsafe { msg_send![v, layer] };
        let Some(layer) = layer else { return 0.0 };
        let n: Option<Retained<AnyObject>> = unsafe { msg_send![&*layer, valueForKey: &*NSString::from_str(key)] };
        n.map(|n| unsafe { msg_send![&*n, doubleValue] }).unwrap_or(0.0)
    }

    struct App {
        backend: AppKitBackend,
        host: Host<'static>,
        win: Retained<AnyObject>,
    }

    impl App {
        fn open() -> App {
            let bundle = root().join("fixtures/todo-mac/dist/app.js");
            assert!(bundle.is_file(), "{} is missing: run `pnpm install && pnpm -r build` first", bundle.display());
            let backend = AppKitBackend::new().unwrap();
            let vt = backend.vtable();
            let bref = unsafe { BackendRef::from_raw(vt.as_raw()) }.unwrap();
            let host = Host::new(bref, HostConfig::default()).unwrap();
            backend.attach_host(host.host_callbacks());
            host.load_bundle(&bundle).unwrap();
            pump(150);
            let win = backend.object(*backend.toplevels().last().expect("a window")).unwrap();
            let _: () = unsafe { msg_send![&*win, makeKeyAndOrderFront: Option::<&AnyObject>::None] };
            pump(50);
            let app = App { backend, host, win };
            app.no_errors();
            app
        }

        fn no_errors(&self) {
            let errors = self.host.take_errors();
            assert!(errors.is_empty(), "JS errors: {errors:?}");
        }

        fn objects(&self) -> Vec<Retained<AnyObject>> {
            self.backend.handles().into_iter().filter_map(|h| self.backend.object(h)).collect()
        }

        /// Live objects of `class` or a subclass (KVO swaps observed objects to `NSKVONotifying_*`).
        fn of_class(&self, class: &str) -> Vec<Retained<AnyObject>> {
            let cls = objc2::runtime::AnyClass::get(&std::ffi::CString::new(class).unwrap()).unwrap();
            self.objects().into_iter().filter(|o| unsafe { msg_send![&**o, isKindOfClass: cls] }).collect()
        }

        fn label(&self, text: &str) -> Retained<AnyObject> {
            self.of_class("NSTextField")
                .into_iter()
                .find(|o| string_of(o, "stringValue") == text)
                .unwrap_or_else(|| panic!("no text field {text:?}: {:?}", self.labels()))
        }

        fn labels(&self) -> Vec<String> {
            self.of_class("NSTextField").iter().map(|o| string_of(o, "stringValue")).collect()
        }

        fn button(&self, title: &str) -> Retained<AnyObject> {
            self.of_class("NSButton")
                .into_iter()
                .find(|o| string_of(o, "title") == title)
                .unwrap_or_else(|| panic!("no button {title:?}"))
        }

        fn click(&self, o: &AnyObject) {
            let _: () = unsafe { msg_send![o, performClick: Option::<&AnyObject>::None] };
            pump(80);
        }

        fn field(&self) -> Retained<AnyObject> {
            self.of_class("NSTextField")
                .into_iter()
                .find(|o| string_of(o, "placeholderString") == "What needs doing?")
                .expect("the new-todo field")
        }

        /// Types into the new-todo field the way AppKit reports typing (the change notification).
        fn type_text(&self, text: &str) {
            let f = self.field();
            let s = NSString::from_str(text);
            let name = NSString::from_str("NSControlTextDidChangeNotification");
            unsafe {
                let _: () = msg_send![&*f, setStringValue: &*s];
                let nc: Retained<AnyObject> = msg_send![class!(NSNotificationCenter), defaultCenter];
                let _: () = msg_send![&*nc, postNotificationName: &*name, object: &*f];
            }
            pump(80);
        }

        /// The row stack view (parent of a label).
        fn row_of(&self, label: &AnyObject) -> Retained<AnyObject> {
            unsafe { msg_send![label, superview] }
        }

        fn alpha(&self, v: &AnyObject) -> f64 {
            unsafe { msg_send![v, alphaValue] }
        }

        fn struck(&self, label: &AnyObject) -> bool {
            let a: Retained<AnyObject> = unsafe { msg_send![label, attributedStringValue] };
            let len: usize = unsafe { msg_send![&*a, length] };
            if len == 0 {
                return false;
            }
            let key = NSString::from_str("NSStrikethrough");
            let v: Option<Retained<AnyObject>> = unsafe { msg_send![&*a, attribute: &*key, atIndex: 0usize, effectiveRange: std::ptr::null_mut::<objc2_foundation::NSRange>()] };
            v.is_some_and(|n| {
                let i: isize = unsafe { msg_send![&*n, integerValue] };
                i != 0
            })
        }

        fn screenshot(&self, name: &str) {
            let dir = root().join("target/todo");
            std::fs::create_dir_all(&dir).unwrap();
            let png = dir.join(format!("{name}.png"));
            let _ = std::fs::remove_file(&png);
            let n: isize = unsafe { msg_send![&*self.win, windowNumber] };
            let p = png.clone();
            let t = std::thread::spawn(move || std::process::Command::new("screencapture").args(["-x", "-o", &format!("-l{n}")]).arg(&p).status().is_ok_and(|s| s.success()));
            while !t.is_finished() {
                pump(20);
            }
            assert!(t.join().unwrap() && png.is_file(), "no screenshot at {} (Screen Recording permission?)", png.display());
        }

        fn close(self) {
            self.no_errors();
            let _: () = unsafe { msg_send![&*self.win, orderOut: Option::<&AnyObject>::None] };
            pump(20);
        }
    }

    pub fn styles_applied() {
        let app = App::open();
        // .title: 28 pt, weight 700
        let title = app.label("Todos");
        let font: Retained<AnyObject> = unsafe { msg_send![&*title, font] };
        let size: f64 = unsafe { msg_send![&*font, pointSize] };
        assert_eq!(size, 28.0, "font-size");
        let desc: Retained<AnyObject> = unsafe { msg_send![&*font, fontDescriptor] };
        let traits: u32 = unsafe { msg_send![&*desc, symbolicTraits] };
        assert!(traits & 2 != 0, "font-weight 700 is bold (traits {traits:#x})");
        // .card: layer background, 1 px border, radius 10, a shadow
        let field = app.field();
        let add_row: Retained<AnyObject> = unsafe { msg_send![&*field, superview] };
        let card: Retained<AnyObject> = unsafe { msg_send![&*add_row, superview] };
        assert_eq!(class_name(&card), "NSStackView");
        assert!(layer_bg(&card).is_some(), "card background");
        assert_eq!(layer_f64(&card, "cornerRadius"), 10.0, "border-radius");
        assert_eq!(layer_f64(&card, "borderWidth"), 1.0, "border width");
        assert_eq!(layer_f64(&card, "shadowOpacity"), 1.0, "box-shadow");
        // .primary: flat (borderless, layer background), :disabled opacity while the draft is empty
        let add = app.button("Add");
        let bordered: bool = unsafe { msg_send![&*add, isBordered] };
        assert!(!bordered, "a button with background and radius is flat");
        assert!(layer_bg(&add).is_some(), "the flat button's layer background");
        assert!((app.alpha(&add) - 0.4).abs() < 1e-6, ":disabled opacity 0.4, got {}", app.alpha(&add));
        // .row.done .label: line-through (descendant selector + class: toggle)
        assert!(app.struck(&app.label("Try svelters on a Mac")), "the done item is struck through");
        assert!(!app.struck(&app.label("Ship it")), "an open item is not");
        // .delete: a red title on a push button
        let del = app.button("Delete");
        let at: Retained<AnyObject> = unsafe { msg_send![&*del, attributedTitle] };
        let key = NSString::from_str("NSColor");
        let c: Option<Retained<AnyObject>> = unsafe { msg_send![&*at, attribute: &*key, atIndex: 0usize, effectiveRange: std::ptr::null_mut::<objc2_foundation::NSRange>()] };
        let c = rgba(c.as_deref()).expect("the delete title has a colour");
        assert!(c[0] > 0.6 && c[1] < 0.5, "danger red, got {}", hex(c));
        // .empty / align-self etc. are covered by the filter case; .new: flex-grow fills the row
        let fw = frame(&field).size.width;
        assert!(fw > 300.0, "flex-grow stretches the field: width {fw}");
        app.screenshot("todo-initial");
        app.close();
    }

    pub fn add_toggle_delete() {
        let app = App::open();
        let add = app.button("Add");
        app.type_text("Write the docs");
        assert!((app.alpha(&add) - 1.0).abs() < 1e-6, "Add enabled: :disabled block off, alpha {}", app.alpha(&add));
        app.click(&add);
        let l = app.label("Write the docs");
        assert!(app.labels().contains(&"3 items left".to_string()), "{:?}", app.labels());
        assert!(!app.struck(&l));
        // the row's checkbox: class:done switches the descendant rule on
        let row = app.row_of(&l);
        let parts: Retained<AnyObject> = unsafe { msg_send![&*row, arrangedSubviews] };
        let check: Retained<AnyObject> = unsafe { msg_send![&*parts, objectAtIndex: 0usize] };
        app.click(&check);
        assert!(app.struck(&app.label("Write the docs")), "done -> line-through");
        app.click(&check);
        assert!(!app.struck(&app.label("Write the docs")), "not done -> plain again (attributes dropped)");
        // delete it
        let del: Retained<AnyObject> = unsafe { msg_send![&*parts, objectAtIndex: 2usize] };
        app.click(&del);
        assert!(!app.labels().iter().any(|s| s == "Write the docs"), "deleted");
        // filter Done, clear completed: the empty state
        app.click(&app.button("Clear completed"));
        let seg = app.of_class("NSSegmentedControl").pop().unwrap();
        let _: () = unsafe { msg_send![&*seg, setSelectedSegment: 2isize] };
        let target: Option<Retained<AnyObject>> = unsafe { msg_send![&*seg, target] };
        let action: Option<objc2::runtime::Sel> = unsafe { msg_send![&*seg, action] };
        let nsapp: Retained<AnyObject> = unsafe { msg_send![class!(NSApplication), sharedApplication] };
        let _: bool = unsafe { msg_send![&*nsapp, sendAction: action.unwrap(), to: target.as_deref(), from: &*seg] };
        pump(80);
        let empty = app.label("Nothing done yet");
        let align: isize = unsafe { msg_send![&*empty, alignment] };
        let center = if cfg!(target_arch = "x86_64") { 2 } else { 1 };
        assert_eq!(align, center, "text-align: center (NSTextAlignment differs by architecture)");
        app.close();
    }

    pub fn hover_block() {
        let app = App::open();
        let row = app.row_of(&app.label("Ship it"));
        assert!(layer_bg(&row).is_none_or(|c| c == "#000000"), "no hover background before: {:?}", layer_bg(&row));
        // the tracking area's owner gets mouseEntered:/mouseExited: from AppKit
        let areas: Retained<AnyObject> = unsafe { msg_send![&*row, trackingAreas] };
        let n: usize = unsafe { msg_send![&*areas, count] };
        assert!(n >= 1, "a tracking area for the :hover block");
        let area: Retained<AnyObject> = unsafe { msg_send![&*areas, objectAtIndex: n - 1] };
        let owner: Retained<AnyObject> = unsafe { msg_send![&*area, owner] };
        let _: () = unsafe { msg_send![&*owner, mouseEntered: Option::<&AnyObject>::None] };
        pump(20);
        let on = layer_bg(&row);
        assert!(on.as_deref().is_some_and(|c| c != "#000000"), ":hover background on: {on:?}");
        let _: () = unsafe { msg_send![&*owner, mouseExited: Option::<&AnyObject>::None] };
        pump(20);
        assert!(layer_bg(&row).is_none_or(|c| c == "#000000"), ":hover background off: {:?}", layer_bg(&row));
        app.close();
    }

    pub fn scheme_switch() {
        let app = App::open();
        let nsapp: Retained<AnyObject> = unsafe { msg_send![class!(NSApplication), sharedApplication] };
        let field = app.field();
        let add_row: Retained<AnyObject> = unsafe { msg_send![&*field, superview] };
        let card: Retained<AnyObject> = unsafe { msg_send![&*add_row, superview] };
        let toggle = app.of_class("NSButton").into_iter().find(|o| matches!(string_of(o, "title").as_str(), "Dark" | "Light")).unwrap();
        let before_title = string_of(&toggle, "title");
        let before = layer_bg(&card);
        app.click(&toggle);
        pump(50);
        let after = layer_bg(&card);
        let ap: Option<Retained<AnyObject>> = unsafe { msg_send![&*nsapp, appearance] };
        let name = ap.map(|a| string_of(&a, "name")).unwrap_or_default();
        let now_dark = before_title == "Dark";
        assert_eq!(name.contains("Dark"), now_dark, "NSApp appearance follows the scheme: {name}");
        // --sv-surface: #ffffff light, #303030 dark
        assert_eq!(after.as_deref(), Some(if now_dark { "#303030" } else { "#ffffff" }), "card surface token after the switch (before {before:?})");
        app.screenshot(if now_dark { "todo-dark" } else { "todo-light" });
        app.click(&app.of_class("NSButton").into_iter().find(|o| matches!(string_of(o, "title").as_str(), "Dark" | "Light")).unwrap());
        app.screenshot(if now_dark { "todo-light" } else { "todo-dark" });
        let _: () = unsafe { msg_send![&*nsapp, setAppearance: Option::<&AnyObject>::None] };
        app.close();
    }

    pub fn list_scrolls() {
        let app = App::open();
        for i in 0..20 {
            app.type_text(&format!("task {i}"));
            app.click(&app.button("Add"));
        }
        let first = app.row_of(&app.label("task 0"));
        let last = app.row_of(&app.label("task 19"));
        let h0 = frame(&first).size.height;
        let h1 = frame(&last).size.height;
        assert!((h0 - h1).abs() < 0.5, "rows keep one height (flex-grow: 0): {h0} vs {h1}");
        let scroll = app.of_class("NSScrollView").pop().expect("the list's scroll view");
        let clip: Retained<AnyObject> = unsafe { msg_send![&*scroll, contentView] };
        let doc: Retained<AnyObject> = unsafe { msg_send![&*scroll, documentView] };
        let ch = frame(&clip).size.height;
        let dh = frame(&doc).size.height;
        assert!(dh > ch, "the list is taller than its scroll view ({dh} > {ch})");
        let flipped: bool = unsafe { msg_send![&*doc, isFlipped] };
        assert!(flipped, "the stack view sits in a flipped document view (content starts at the top)");
        app.screenshot("todo-many");
        app.close();
    }
}
