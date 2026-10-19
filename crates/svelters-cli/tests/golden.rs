//! Golden output tests for `svelters docs` (markdown, `--brief`, `--json`) on the checked-in IR
//! subset in tests/fixtures/ir (regenerate it with tests/fixtures/make_fixture.py). Each golden
//! file holds the exit code, stdout and stderr. `UPDATE_GOLDEN=1` rewrites them.

use std::path::{Path, PathBuf};

use svelters_cli::{Ctx, run};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixtures() -> PathBuf {
    crate_dir().join("tests/fixtures/ir")
}

fn ctx(cwd: &Path) -> Ctx {
    Ctx {
        cwd: cwd.to_path_buf(),
        env_packs: vec![],
        explicit_only: true,
        color: false,
    }
}

/// Runs `svelters docs --ir <fixtures> <args…>` and renders the result as one text.
fn docs_in(cwd: &Path, args: &str) -> (i32, String) {
    let mut argv: Vec<String> = vec![
        "docs".into(),
        "--ir".into(),
        fixtures().display().to_string(),
    ];
    argv.extend(args.split_whitespace().map(String::from));
    let out = run(&argv, &ctx(cwd));
    let fx = fixtures().display().to_string();
    let text = format!(
        "$ svelters docs {args}\nexit: {}\n--- stdout\n{}--- stderr\n{}",
        out.code, out.stdout, out.stderr
    )
    .replace(&fx, "<fixtures>");
    (out.code, text)
}

fn docs(args: &str) -> (i32, String) {
    docs_in(&crate_dir(), args)
}

fn golden(name: &str, args: &str, want_code: i32) {
    let (code, text) = docs(args);
    let path = crate_dir().join("tests/golden").join(format!("{name}.txt"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &text).unwrap();
    }
    let want = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("{} missing; run with UPDATE_GOLDEN=1", path.display()));
    assert_eq!(
        text,
        want,
        "{name} differs from {} (UPDATE_GOLDEN=1 rewrites)",
        path.display()
    );
    assert_eq!(code, want_code, "{name}: exit code");
    if args.contains("--json") {
        let json = text
            .split("--- stdout\n")
            .nth(1)
            .unwrap()
            .split("--- stderr\n")
            .next()
            .unwrap();
        let v: serde_json::Value =
            serde_json::from_str(json).expect("--json prints one JSON document");
        assert_eq!(v["schema"], "svelters-docs/1");
        assert!(v["kind"].is_string() && v["notes"].is_array());
    }
}

macro_rules! goldens {
    ($($name:ident: $args:expr => $code:expr;)*) => {
        $(#[test] fn $name() { golden(stringify!($name), $args, $code); })*
    };
}

goldens! {
    toolkits: "" => 0;
    toolkits_json: "--json" => 0;
    index_gtk: "gtk" => 0;
    index_qt_all_json: "qt --all --json" => 0;
    // element cards
    gtk_button_md: "gtk button" => 0;
    gtk_button_brief: "gtk button --brief" => 0;
    gtk_button_json: "gtk button --json" => 0;
    gtk_button_brief_json: "gtk button --brief --json" => 0;
    gtk_button_all_props: "gtk button --all --section properties" => 0;
    gtk_application_window_md: "gtk application-window" => 0;
    qt_main_window_md: "qt main-window" => 0;
    qt_main_window_brief: "qt main-window --brief" => 0;
    qt_main_window_json: "qt main-window --json" => 0;
    qt_enum_md: "qt QMainWindow::DockOption" => 0;
    // members, incl. inherited and dangerous
    gtk_button_label_md: "gtk button.label" => 0;
    gtk_button_visible_inherited_md: "gtk button.visible" => 0;
    gtk_button_visible_inherited_json: "gtk button.visible --json" => 0;
    gtk_button_onclicked_md: "gtk button.onclicked" => 0;
    gtk_button_child_slot_md: "gtk button.child" => 0;
    gtk_window_set_titlebar_dangerous_md: "gtk window.set_titlebar" => 0;
    gtk_window_set_titlebar_dangerous_json: "gtk window.method:set_titlebar --json" => 0;
    gtk_application_window_titlebar_inherited_md: "gtk application-window.titlebar" => 0;
    gtk_grid_attr_md: "gtk grid.attr:row" => 0;
    gtk_native_names_md: "Gtk.Button.can-shrink" => 0;
    qt_push_button_text_md: "qt push-button.text" => 0;
    qt_push_button_text_json: "qt push-button.text --json" => 0;
    qt_push_button_clicked_brief: "qt push-button.clicked --brief" => 0;
    qt_setcentralwidget_dangerous_md: "QMainWindow::setCentralWidget" => 0;
    qt_resize_overloads_json: "qt main-window.method:resize --json" => 0;
    qt_enum_value_md: "qt QMainWindow::DockOption.AnimatedDocks" => 0;
    // ambiguity, misses, search
    gtk_button_activate_ambiguous: "gtk button.activate" => 2;
    gtk_button_activate_ambiguous_json: "gtk button.activate --json" => 2;
    window_unique_toolkit: "window --brief" => 0;
    qt_only_name_note: "main-window.animated --brief" => 0;
    qt_close_event_vs_method_ambiguous: "qt main-window.close" => 2;
    miss_entity: "gtk butonn" => 1;
    miss_member: "gtk button.lable" => 1;
    miss_member_json: "qt main-window.titel --json" => 1;
    search_dock: "-k dock" => 0;
    search_dock_json: "-k title --json --limit 5" => 0;
}

#[test]
fn usage_errors_exit_3() {
    assert_eq!(docs("nosuch button").0, 3);
    assert_eq!(docs("gtk button extra").0, 3);
    assert_eq!(docs("--bogus").0, 3);
    assert_eq!(docs("gtk button --section nope").0, 3);
}

#[test]
fn help_and_version() {
    let out = run(&["docs".into(), "--help".into()], &ctx(&crate_dir()));
    assert_eq!(out.code, 0);
    assert!(out.stdout.contains("EXIT STATUS"));
    let out = run(&["--version".into()], &ctx(&crate_dir()));
    assert!(out.stdout.starts_with("svelters "));
}

#[test]
fn no_data_exits_4() {
    let out = run(
        &["docs".into(), "gtk".into(), "button".into()],
        &ctx(&crate_dir()),
    );
    assert_eq!(out.code, 4, "{}", out.stderr);
    assert!(out.stderr.contains("no docs packs"));
}

/// A project directory with `svelters.toml` picks the toolkit; naming another prints a note.
#[test]
fn project_toolkit_from_svelters_toml() {
    let dir = std::env::temp_dir().join(format!("svelters-cli-test-{}", std::process::id()));
    let sub = dir.join("src/lib");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(
        dir.join("svelters.toml"),
        "[app]\nname = \"t\"\ntoolkit = \"qt\" # one per app\n",
    )
    .unwrap();

    // `close` exists in both toolkits' main windows… but only qt is searched
    let (code, text) = docs_in(&sub, "main-window.event:close --brief");
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("on <main-window>"), "{text}");
    assert!(!text.contains("found only in"), "{text}");
    // a name that only GTK has is not found in the project's toolkit
    let (code, text) = docs_in(&sub, "application-window");
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("in qt."), "{text}");
    // explicit other toolkit: answered, with a note
    let (code, text) = docs_in(&sub, "gtk button --brief");
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("note: this app targets qt"), "{text}");
    // the toolkits listing names the project
    let (_, text) = docs_in(&sub, "--json");
    assert!(text.contains("\"toolkit\": \"qt\""), "{text}");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Packs written by docs-pack open again, and `--pack` reads them.
#[test]
fn docs_pack_writes_and_checks() {
    let dir = std::env::temp_dir().join(format!("svelters-cli-pack-{}", std::process::id()));
    let c = ctx(&crate_dir());
    let args = |extra: &[&str]| -> Vec<String> {
        let mut v = vec![
            "docs-pack".to_string(),
            "--ir".into(),
            fixtures().display().to_string(),
            "--out".into(),
            dir.display().to_string(),
        ];
        v.extend(extra.iter().map(|s| s.to_string()));
        v
    };
    assert_eq!(
        run(&args(&["--check"]), &c).code,
        1,
        "missing packs are stale"
    );
    let out = run(&args(&[]), &c);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(dir.join("gtk.svpack").is_file() && dir.join("qt.svpack").is_file());
    assert_eq!(run(&args(&["--check"]), &c).code, 0);
    let out = run(
        &[
            "docs".into(),
            "--pack".into(),
            dir.display().to_string(),
            "qt".into(),
            "push-button.text".into(),
            "--json".into(),
        ],
        &c,
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(
        out.stdout
            .contains("\"address\": \"qt push-button.prop:text\""),
        "{}",
        out.stdout
    );
    // a damaged pack is reported, not a panic
    std::fs::write(dir.join("bad.svpack"), b"nope").unwrap();
    let out = run(
        &[
            "docs".into(),
            "--pack".into(),
            dir.join("bad.svpack").display().to_string(),
        ],
        &c,
    );
    assert_eq!(out.code, 4);
    assert!(out.stderr.contains("bad magic"), "{}", out.stderr);
    std::fs::remove_dir_all(&dir).unwrap();
}
