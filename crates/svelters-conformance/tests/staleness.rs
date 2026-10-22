//! NA5: no stale goldens. Every golden under `conformance/golden/<platform>/` must hold every pin of
//! `conformance/pins/`, each with the fingerprint the pin has today (harness version, pin file and
//! the chords of the platform's key table); a changed pin or key table needs a re-capture
//! (`NA_UPDATE=1`, see plans/milestones/WG3-native-references.md). A pin that cannot run on the
//! platform (`pins::not_applicable`, WG3m) must be absent and listed under `not_applicable` with
//! today's reason. Runs anywhere: no toolkit.

use svelters_conformance::golden::{goldens, read_golden};
use svelters_conformance::pins::{
    Chord, Keys, conformance_dir, fingerprint, load_pins, not_applicable, parse_pin,
};
use svelters_conformance::trace::{State, scalar_to_utf16, utf16_to_scalar};

#[test]
fn goldens_are_current() {
    let pins = load_pins().unwrap();
    let mut problems = Vec::new();
    let mut seen = 0;
    let dir = conformance_dir().join("golden");
    for platform in std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
    {
        let platform = platform.file_name().to_string_lossy().into_owned();
        let keys = Keys::load(&platform).unwrap();
        for g in goldens(&platform).unwrap() {
            seen += 1;
            let name = g.path.file_name().unwrap().to_string_lossy().into_owned();
            assert_eq!(
                name,
                format!("{}-{}.json", g.toolkit, g.version),
                "golden file name"
            );
            for p in &pins {
                let listed = g.not_applicable.iter().find(|(id, _)| *id == p.id);
                if let Some(why) = not_applicable(p, &keys) {
                    if g.pin(&p.id).is_some() {
                        problems.push(format!(
                            "{platform}/{name}: pin {} cannot run there ({why}) but is captured",
                            p.id
                        ));
                    }
                    if listed.map(|(_, r)| r) != Some(&why) {
                        problems.push(format!(
                            "{platform}/{name}: pin {} not listed as not applicable ({why})",
                            p.id
                        ));
                    }
                    continue;
                }
                if listed.is_some() {
                    problems.push(format!(
                        "{platform}/{name}: pin {} is listed as not applicable but runs",
                        p.id
                    ));
                }
                match g.pin(&p.id) {
                    None => problems.push(format!("{platform}/{name}: pin {} not captured", p.id)),
                    Some(r) if r.fingerprint != fingerprint(p, &keys) => {
                        problems.push(format!("{platform}/{name}: pin {} is stale", p.id))
                    }
                    Some(r) if r.checkpoints.len() != p.actions.len() + 1 => {
                        problems.push(format!(
                            "{platform}/{name}: pin {} has {} checkpoints",
                            p.id,
                            r.checkpoints.len()
                        ))
                    }
                    _ => {}
                }
            }
            for r in &g.pins {
                if !pins.iter().any(|p| p.id == r.id) {
                    problems.push(format!("{platform}/{name}: pin {} no longer exists", r.id));
                }
            }
        }
    }
    assert!(seen > 0, "no goldens under {}", dir.display());
    assert!(
        problems.is_empty(),
        "stale goldens (re-capture with NA_UPDATE=1):\n  {}",
        problems.join("\n  ")
    );
}

#[test]
fn golden_files_round_trip() {
    for platform in ["linux", "macos"] {
        for g in goldens(platform).unwrap() {
            let again = read_golden(&g.path).unwrap();
            assert_eq!(again.pins.len(), g.pins.len());
        }
    }
}

/// WG3m: every pin runs on Linux; on macOS the CUA chords, the second redo chord, Ctrl+Shift+U and
/// the primary selection do not exist, so those four scripts do not apply.
#[test]
fn pins_not_applicable_per_platform() {
    let pins = load_pins().unwrap();
    let na = |platform: &str| {
        let keys = Keys::load(platform).unwrap();
        pins.iter()
            .filter_map(|p| not_applicable(p, &keys).map(|r| format!("{}: {r}", p.id)))
            .collect::<Vec<_>>()
    };
    assert_eq!(na("linux"), Vec::<String>::new());
    assert_eq!(
        na("macos"),
        [
            "undo-typing: no chord on macos for redo-alt",
            "clipboard-keys: no chord on macos for copy-alt, cut-alt, paste-alt",
            "primary: no primary selection on macos",
            "unicode-entry: no chord on macos for unicode-entry",
        ]
    );
}

#[test]
fn pins_parse_and_expand() {
    let p = parse_pin(
        r#"{"id":"x","pin":2,"setup":{"fields":{"a":{"text":"héllo 😀","sel":[1,-1]}},"focus":"a"},
            "steps":[{"key":"word-next","shift":true,"repeat":3},{"click":"c"},{"drag":"a","from":1,"to":4,"count":2}]}"#,
    )
    .unwrap();
    assert_eq!(p.actions.len(), 5);
    assert_eq!(p.setup.field("a").sel(), [1, 7], "-1 is the end, in chars");
    assert_eq!(p.setup.field("b").text, "");
    assert_eq!(p.actions[0].label(), "shift+word-next");
    assert_eq!(p.actions[3].label(), "click c x1");
    assert_eq!(p.actions[4].label(), "drag a 1->4 x2");
    assert!(
        parse_pin(r#"{"id":"y","pin":1,"setup":{"fields":{"z":{}}},"steps":[]}"#).is_err(),
        "unknown field"
    );
    let keys = Keys::load("linux").unwrap();
    assert_eq!(
        keys.chord("word-next", true).unwrap(),
        Chord {
            ctrl: true,
            shift: true,
            key: "Right".into(),
            ..Chord::default()
        }
    );
    assert_eq!(
        Keys::load("macos")
            .unwrap()
            .chord("line-start", false)
            .unwrap()
            .text(),
        "Cmd+Left"
    );
}

#[test]
fn offsets_and_state_json() {
    let t = "a😀b";
    assert_eq!(scalar_to_utf16(t, 2), 3);
    assert_eq!(utf16_to_scalar(t, 3), 2);
    assert_eq!(utf16_to_scalar(t, 99), 3);
    let s = State {
        text: Some("a\"b".into()),
        sel: Some([2, 0]),
        focus: true,
    };
    assert_eq!(
        s.json(),
        r#"{"text":"a\"b","sel":[2,0],"caret":0,"focus":true}"#
    );
    assert_eq!(
        State::from_json(&serde_json::from_str(&s.json()).unwrap()).unwrap(),
        s
    );
    assert_eq!(
        State {
            text: None,
            sel: None,
            focus: false
        }
        .json(),
        r#"{"focus":false}"#
    );
}
