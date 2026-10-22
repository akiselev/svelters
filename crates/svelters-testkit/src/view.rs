//! What expectations look at: the visible nodes of a backend in tree order, each with the text it
//! shows. Every backend maps its own tree onto this (the log backend from its op log, egui from
//! AccessKit, GTK and Qt from the native widget dump of `examples/counter`).

use crate::case::Expect;
use serde_json::Value as Json;

#[derive(Clone, Debug, PartialEq)]
pub struct ViewNode {
    pub depth: u32,
    /// The backend's name for the node (element kind, toolkit class or AccessKit role).
    pub kind: String,
    /// The text it shows; empty for containers.
    pub text: String,
}

/// The native widget dump of `examples/counter` (`[{d, class, text}]`, layout order).
pub fn from_dump(dump: &Json) -> Vec<ViewNode> {
    dump.as_array()
        .map(|a| {
            a.iter()
                .map(|n| ViewNode {
                    depth: n["d"].as_u64().unwrap_or(0) as u32,
                    kind: n["class"].as_str().unwrap_or("").to_string(),
                    text: n["text"].as_str().unwrap_or("").to_string(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Checks one expectation against `view`.
pub fn check(e: &Expect, view: &[ViewNode]) -> Result<(), String> {
    let shown = |t: &str| view.iter().position(|n| n.text == t);
    match e {
        Expect::Text(t) => shown(t).map(|_| ()).ok_or_else(|| format!("no node shows {t:?}; shown: {}", texts(view))),
        Expect::NoText(t) => match shown(t) {
            Some(_) => Err(format!("{t:?} is shown; shown: {}", texts(view))),
            None => Ok(()),
        },
        Expect::Order(want) => {
            let mut from = 0;
            for t in want {
                match view[from..].iter().position(|n| &n.text == t) {
                    Some(i) => from += i + 1,
                    None if shown(t).is_some() => return Err(format!("{t:?} is out of order (want {want:?}); shown: {}", texts(view))),
                    None => return Err(format!("no node shows {t:?} (want order {want:?}); shown: {}", texts(view))),
                }
            }
            Ok(())
        }
    }
}

/// The non-empty texts of `view`, quoted, for messages.
pub fn texts(view: &[ViewNode]) -> String {
    let t: Vec<String> = view.iter().filter(|n| !n.text.is_empty()).map(|n| format!("{:?}", n.text)).collect();
    format!("[{}]", t.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(texts: &[&str]) -> Vec<ViewNode> {
        texts.iter().map(|t| ViewNode { depth: 0, kind: "label".into(), text: t.to_string() }).collect()
    }

    #[test]
    fn expectations() {
        let view = v(&["", "a", "b", "c"]);
        assert!(check(&Expect::Text("b".into()), &view).is_ok());
        assert!(check(&Expect::Text("x".into()), &view).unwrap_err().contains("no node shows \"x\""));
        assert!(check(&Expect::NoText("x".into()), &view).is_ok());
        assert!(check(&Expect::NoText("a".into()), &view).is_err());
        assert!(check(&Expect::Order(vec!["a".into(), "c".into()]), &view).is_ok());
        assert!(check(&Expect::Order(vec!["c".into(), "a".into()]), &view).unwrap_err().contains("out of order"));
    }

    #[test]
    fn reads_the_counter_dump() {
        let dump = serde_json::json!([{"d": 0, "class": "GtkWindow", "text": "Counter"}, {"d": 2, "class": "GtkLabel", "text": "Count: 3"}]);
        let view = from_dump(&dump);
        assert_eq!(view[1], ViewNode { depth: 2, kind: "GtkLabel".into(), text: "Count: 3".into() });
    }
}
