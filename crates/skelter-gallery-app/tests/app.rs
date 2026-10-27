//! The interactive gallery, headless (skelter-test's `Harness`): the sidebar, mounting a story's
//! cells, previews through the overlay, saving, and a compile error keeping the last good render.

use skelter_core::{KindId, Value};
use skelter_gallery::Extensions;
use skelter_gallery_app::GalleryDocument;
use skelter_test::Harness;
use std::path::{Path, PathBuf};

/// A copy of `tests/fixtures/gallery` under `target/` (the tests edit and save its files). Svelte
/// resolves through `@svelters/native-runtime` (the workspace's, after `pnpm install`).
fn fixture(name: &str) -> PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gallery");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/gallery-app-tests").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    copy(&src, &dir);
    dir.canonicalize().unwrap()
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let p = e.path();
        if p.is_dir() {
            copy(&p, &to.join(e.file_name()));
        } else {
            std::fs::copy(&p, to.join(e.file_name())).unwrap();
        }
    }
}

/// The platform's command modifier (VS Code's keys: Cmd on macOS).
const CMD: skelter_base::event::Modifiers = if cfg!(target_os = "macos") { skelter_base::event::Modifiers::SUPER } else { skelter_base::event::Modifiers::CTRL };

fn gallery(name: &str) -> (Harness<GalleryDocument>, PathBuf) {
    let dir = fixture(name);
    let fonts = skelter_text::FontStore::new(skelter_text::FontConfig::testing());
    let doc = skelter_gallery_app::app::document(Extensions::new().story_root(&dir), fonts).unwrap();
    let mut h = Harness::<GalleryDocument>::builder().with_size(skelter_base::geom::Size::new(1500.0, 950.0)).build(doc);
    h.run();
    (h, dir)
}

/// The text prop of the first node of `kind` under `frame`.
fn text_in(h: &Harness<GalleryDocument>, frame: skelter_base::ids::NodeId, kind: &str, prop: &str) -> Option<String> {
    let g = &h.state().gallery;
    let d = g.document().borrow();
    let n = d.query().within(frame).kind(KindId::of(kind)).first()?;
    match d.get_prop(n, skelter_core::PropId::of(kind, prop, "string")) {
        Some(Value::Str(s)) => Some(s.to_string()),
        _ => None,
    }
}

#[test]
fn the_sidebar_lists_the_fixture_stories_by_family() {
    let (h, _) = gallery("sidebar");
    h.get_by_author_id("story:button");
    h.get_by_author_id("story:note");
    assert!(h.query_by_label("controls").is_some() && h.query_by_label("text").is_some());
    // kinds the registry has and no story names are listed as gaps
    assert!(h.query_by_label("checkbox").is_some(), "coverage gaps are listed");
}

#[test]
fn selecting_a_story_mounts_its_cells() {
    let (mut h, _) = gallery("select");
    h.get_by_author_id("story:button").click();
    h.run();
    let g = &h.state().gallery;
    assert_eq!(g.selection().0, Some("button"));
    // primary: 2 captions, plain: 1 row; × 3 states × 4 themes
    let cells: Vec<_> = g.cells().map(|(c, f)| (c.clone(), f)).collect();
    assert_eq!(cells.len(), 3 * 3 * 4);
    for (c, frame) in &cells {
        let caption = text_in(&h, *frame, "button", "text").unwrap_or_else(|| panic!("cell {} has no button", c.label()));
        let want = c.args.get("caption").and_then(|v| v.as_str()).unwrap_or("Plain");
        assert_eq!(caption, want, "{}", c.label());
    }
    // each theme is its own class on its cell's frame
    let d = g.document().borrow();
    let light = cells.iter().find(|(c, _)| c.theme.name() == "light-modern").unwrap().1;
    let dark = cells.iter().find(|(c, _)| c.theme.name() == "dark-modern").unwrap().1;
    let bg = |n| d.view(n).and_then(|v| v.css()?.custom("--vscode-editor-background").map(|s| s.to_string()));
    assert_ne!(bg(light), bg(dark), "light and dark cells resolve different tokens");
    // the hover cell has hover forced on its subject; disabled is the prop where the variant takes it
    let hover = cells.iter().find(|(c, _)| c.state == skelter_gallery::State::Hover).unwrap().1;
    let subject = d.query().within(hover).anchor("subject").first().unwrap();
    assert!(d.forced_state(subject).contains(svelters_style::StateMask::HOVERED));
    // the source pane shows the first variant's file
    drop(d);
    let (file, text) = h.state().gallery.source().unwrap();
    assert!(file.ends_with("primary.svelte") && text.contains("caption"), "{file:?}");
}

#[test]
fn a_preview_edit_changes_the_cell_without_touching_disk() {
    let (mut h, dir) = gallery("preview");
    h.get_by_author_id("story:note").click();
    h.run();
    let file = dir.join("stories/text/note/plain.svelte");
    let on_disk = std::fs::read_to_string(&file).unwrap();
    let frame = h.state().gallery.cells().next().unwrap().1;
    assert_eq!(text_in(&h, frame, "label", "text").as_deref(), Some("Alpha"));
    // type into the source pane: select all, then the new text
    h.get_by_author_id("source").click();
    h.run();
    h.key_press_modifiers(CMD, skelter_base::key::Key::A);
    h.type_text("<label anchor=\"subject\" text=\"Beta\" />\n");
    h.run();
    let frame = h.state().gallery.cells().next().unwrap().1;
    assert_eq!(text_in(&h, frame, "label", "text").as_deref(), Some("Beta"), "the cell shows the buffer");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), on_disk, "nothing was saved");
    assert!(h.state().gallery.unsaved());
    // Ctrl+S saves
    h.key_press_modifiers(CMD, skelter_base::key::Key::S);
    h.run();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "<label anchor=\"subject\" text=\"Beta\" />\n");
    assert!(!h.state().gallery.unsaved());
    assert_eq!(text_in(&h, frame, "label", "text").as_deref(), Some("Beta"));
}

#[test]
fn a_compile_error_keeps_the_last_good_render_and_shows_where() {
    let (mut h, _) = gallery("error");
    h.get_by_author_id("story:note").click();
    h.run();
    h.get_by_author_id("source").click();
    h.run();
    h.key_press_modifiers(CMD, skelter_base::key::Key::A);
    h.type_text("<label anchor=\"subject\" text=\"Gamma\" \n");
    h.run();
    let g = &h.state().gallery;
    let frame = g.cells().next().unwrap().1;
    assert_eq!(text_in(&h, frame, "label", "text").as_deref(), Some("Alpha"), "the last good render stays");
    let errors = g.live().unwrap().errors().to_vec();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].file.ends_with("plain.svelte") && errors[0].line == 1 && errors[0].column > 1, "{errors:?}");
    // shown in the cell and over the source pane
    let shown = h.get_by_author_id("source-errors").label().unwrap_or_default();
    assert!(shown.contains("stories/text/note/plain.svelte:1:"), "{shown}");
    // fixing it applies again
    h.key_press_modifiers(CMD, skelter_base::key::Key::A);
    h.type_text("<label anchor=\"subject\" text=\"Delta\" />\n");
    h.run();
    let frame = h.state().gallery.cells().next().unwrap().1;
    assert_eq!(text_in(&h, frame, "label", "text").as_deref(), Some("Delta"));
    assert!(h.state().gallery.live().unwrap().errors().is_empty());
}

#[test]
fn a_file_changed_on_disk_reloads_through_the_host() {
    let (mut h, dir) = gallery("disk");
    h.get_by_author_id("story:note").click();
    h.run();
    let file = dir.join("stories/text/note/plain.svelte");
    std::fs::write(&file, "<label anchor=\"subject\" text=\"Agent\" />\n").unwrap();
    // what the watcher does after its debounce (the test drives it; the app's watcher thread wakes the loop)
    let r = h.state().gallery.live().unwrap().embed().host().apply_changes(vec![file.clone()]);
    assert!(r.is_some_and(|r| r.ok));
    h.state_mut().gallery.dirty = true;
    h.run();
    let frame = h.state().gallery.cells().next().unwrap().1;
    assert_eq!(text_in(&h, frame, "label", "text").as_deref(), Some("Agent"));
    assert!(h.state().gallery.source().unwrap().1.contains("Agent"), "the source pane follows the disk");
}

#[test]
fn picking_a_cell_readies_its_shot_command_and_inspect_picks_the_clicked_node() {
    let (mut h, _) = gallery("inspect");
    h.get_by_author_id("story:button").click();
    h.run();
    // cell 5: primary, caption=Save, hover, light-modern (args, then states, then themes fastest)
    let (cell, frame) = h.state().gallery.cells().nth(5).map(|(c, f)| (c.clone(), f)).unwrap();
    assert_eq!((cell.state, cell.theme.name()), (skelter_gallery::State::Hover, "light-modern"));
    h.state_mut().gallery.pick_cell(5);
    h.run();
    h.get_by_author_id("copy-shot").click();
    h.run();
    let copied = h.state().gallery.copied.clone().unwrap();
    assert_eq!(copied, "skelter-gallery shot button/primary --args caption=Save --state hover --theme light-modern --density default --zoom 1 -o target/gallery/button-primary.png");
    // inspect mode: a press on the cell's button picks it and reaches nothing else
    let seen: std::rc::Rc<std::cell::RefCell<Vec<(String, skelter_base::ids::NodeId)>>> = Default::default();
    let s2 = seen.clone();
    h.state_mut().gallery.set_inspector(Box::new(move |_d, _frame, node, cell, sel| {
        s2.borrow_mut().push((sel.to_string(), node));
        serde_json::json!({ "selector": sel, "state": cell.state })
    }));
    h.get_by_author_id("inspect-toggle").click();
    h.run();
    assert!(h.state().gallery.settings.inspect);
    let button = {
        let d = h.state().gallery.document().borrow();
        d.query().within(frame).kind(KindId::of("button")).first().unwrap()
    };
    let r = h.state().gallery.document().borrow().world_rect(button).unwrap();
    h.click_at(skelter_base::geom::Point::new((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0));
    h.run();
    assert_eq!(seen.borrow().as_slice(), [("#subject".to_string(), button)]);
    let want = "skelter-gallery inspect button/primary '#subject' --args caption=Save --state hover --theme light-modern --density default --zoom 1 --json";
    let i = h.state().gallery.inspected.clone();
    assert_eq!(i.json.as_ref().unwrap()["selector"], "#subject");
    assert_eq!(i.command.as_deref(), Some(want));
    h.get_by_author_id("copy-inspect").click();
    h.run();
    assert_eq!(h.state().gallery.copied.as_deref(), Some(want));
}

#[test]
fn the_controls_narrow_the_matrix() {
    let (mut h, _) = gallery("controls");
    h.get_by_author_id("story:button").click();
    h.run();
    assert_eq!(h.state().gallery.cells().count(), 36);
    // two themes off
    h.get_by_author_id("theme:hc-dark").click();
    h.run();
    h.get_by_author_id("theme:hc-light").click();
    h.run();
    assert_eq!(h.state().gallery.cells().count(), 18);
    // one state off
    h.get_by_author_id("state:hover").click();
    h.run();
    assert_eq!(h.state().gallery.cells().count(), 12);
    assert!(h.state().gallery.cells().all(|(c, _)| c.state != skelter_gallery::State::Hover));
    // a variant from the sidebar: only its rows
    h.get_by_author_id("variant:button/plain").click();
    h.run();
    assert_eq!(h.state().gallery.selection(), (Some("button"), Some("plain")));
    assert!(h.state().gallery.cells().all(|(c, _)| c.variant.as_str() == "plain"));
    assert!(h.state().gallery.source().unwrap().0.ends_with("plain.svelte"));
    // the filter hides stories that don't match
    h.get_by_author_id("filter").click();
    h.type_text("note");
    h.run();
    assert!(h.query_by_author_id("story:button").is_none());
    h.get_by_author_id("story:note");
}

#[test]
fn an_edit_to_base_css_restyles_the_cells() {
    let (mut h, dir) = gallery("base-css");
    let css = dir.join("base.css");
    std::fs::write(&css, format!("{}\n", skelter_widgets::theme::BASE_CSS)).unwrap();
    h.state_mut().gallery.set_base_css_path(Some(css.clone()));
    h.get_by_author_id("story:note").click();
    h.run();
    let probe = |h: &Harness<GalleryDocument>| {
        let g = &h.state().gallery;
        let frame = g.cells().next().unwrap().1;
        let d = g.document().borrow();
        let n = d.query().within(frame).kind(KindId::of("label")).first().unwrap();
        d.view(n).and_then(|v| v.css()?.custom("--gallery-probe").map(str::to_string))
    };
    assert_eq!(probe(&h), None);
    std::fs::write(&css, format!("{}\n@layer sv.base {{ label {{ --gallery-probe: 7; }} }}\n", skelter_widgets::theme::BASE_CSS)).unwrap();
    // the app's watch thread wakes the loop; a frame polls the file
    assert!(h.state_mut().gallery.poll_base_css());
    h.run();
    assert_eq!(probe(&h).as_deref(), Some("7"));
}
