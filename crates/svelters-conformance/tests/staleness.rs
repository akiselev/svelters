//! NA5: no stale goldens. Every golden under `conformance/golden/<platform>/` must hold every pin of
//! `conformance/pins/`, each with the fingerprint the pin has today (harness version, pin file and
//! the chords of the platform's key table); a changed pin or key table needs a re-capture
//! (`NA_UPDATE=1`, see plans/milestones/WG3-native-references.md). Runs anywhere: no toolkit.

use svelters_conformance::golden::{goldens, read_golden};
use svelters_conformance::pins::{Chord, Keys, conformance_dir, fingerprint, load_pins, parse_pin};
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
    for platform in ["linux", "macos", "windows"] {
        for g in goldens(platform).unwrap() {
            let again = read_golden(&g.path).unwrap();
            assert_eq!(again.pins.len(), g.pins.len());
        }
    }
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
