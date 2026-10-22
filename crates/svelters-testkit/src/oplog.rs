//! A native tree rebuilt from an op log (`plans/architecture/backend-abi.md` "Op-log format"), the
//! way `tools/oplog-compare.mjs` replays one: live handles with kind, props, ordered children,
//! listened signals and observed props. The log backend's runner reads its view and resolves
//! `click`/`type` targets through it.

use crate::view::ViewNode;
use serde_json::Value as Json;
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, Default)]
pub struct Node {
    pub kind: String,
    pub props: BTreeMap<String, Json>,
    pub parent: Option<u64>,
    /// Children in order, all slots together (slots in first-insert order).
    pub children: Vec<u64>,
    pub listening: HashSet<String>,
    pub observing: HashSet<String>,
}

/// Props that hold the text a node shows, by preference.
pub const TEXT_PROPS: [&str; 4] = ["label", "text", "title", "window-title"];
/// Props that name a text input (its placeholder).
pub const FIELD_PROPS: [&str; 3] = ["placeholder-text", "hint-text", "accessible-name"];

impl Node {
    pub fn str_prop(&self, p: &str) -> Option<&str> {
        self.props.get(p).and_then(Json::as_str)
    }

    /// The text the node shows (see [`TEXT_PROPS`]).
    pub fn text(&self) -> &str {
        TEXT_PROPS.iter().find_map(|p| self.str_prop(p)).unwrap_or("")
    }
}

#[derive(Clone, Debug, Default)]
pub struct Mirror {
    pub nodes: BTreeMap<u64, Node>,
}

fn handle(s: &str) -> Result<u64, String> {
    s.trim_start_matches('#').parse().map_err(|_| format!("bad handle {s}"))
}

impl Mirror {
    /// Replays `log`. Lines this mirror has no state for (`event`, `call`, `style`, `sync`, ...)
    /// are skipped.
    pub fn replay(log: &str) -> Result<Mirror, String> {
        let mut m = Mirror::default();
        for (i, line) in log.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
            m.apply(line).map_err(|e| format!("op log line {}: {e}: {line}", i + 1))?;
        }
        Ok(m)
    }

    fn node(&mut self, h: u64) -> Result<&mut Node, String> {
        self.nodes.get_mut(&h).ok_or_else(|| format!("unknown handle #{h}"))
    }

    fn unlink(&mut self, h: u64) {
        if let Some(p) = self.nodes.get_mut(&h).and_then(|n| n.parent.take()) {
            if let Some(pn) = self.nodes.get_mut(&p) {
                pn.children.retain(|c| *c != h);
            }
        }
    }

    fn kill(&mut self, h: u64) {
        self.unlink(h);
        if let Some(n) = self.nodes.remove(&h) {
            for c in n.children {
                if let Some(cn) = self.nodes.get_mut(&c) {
                    cn.parent = None;
                }
            }
        }
    }

    fn apply(&mut self, line: &str) -> Result<(), String> {
        let w: Vec<&str> = line.splitn(4, ' ').collect();
        let arg = |i: usize| w.get(i).copied().ok_or_else(|| "missing field".to_string());
        match w[0] {
            "create" => {
                self.nodes.insert(handle(arg(2)?)?, Node { kind: arg(1)?.to_string(), ..Default::default() });
            }
            "construct" => {
                self.nodes.insert(handle(arg(1)?)?, Node { kind: arg(2)?.to_string(), ..Default::default() });
            }
            "insert" => {
                let w: Vec<&str> = line.split(' ').collect();
                let (p, c) = (handle(w[1])?, handle(w[2])?);
                let before = match w.get(3).and_then(|b| b.strip_prefix("before=")) {
                    Some("end") | None => None,
                    Some(b) => Some(handle(b)?),
                };
                self.unlink(c);
                self.node(c)?.parent = Some(p);
                let pn = self.node(p)?;
                let at = before.and_then(|b| pn.children.iter().position(|x| *x == b)).unwrap_or(pn.children.len());
                pn.children.insert(at, c);
            }
            "remove" => self.unlink(handle(arg(2)?)?),
            "destroy" => self.kill(handle(arg(1)?)?),
            // `gone #h reason`: the native object died; its descendants report their own lines
            "gone" => self.kill(handle(arg(1)?)?),
            "set" | "changed" => {
                let h = handle(arg(1)?)?;
                let prop = arg(2)?.to_string();
                let v: Json = match w.get(3) {
                    Some(j) if !j.is_empty() => serde_json::from_str(j).map_err(|e| e.to_string())?,
                    _ => return Ok(()),
                };
                let n = self.node(h)?;
                if v.is_null() {
                    n.props.remove(&prop);
                } else {
                    n.props.insert(prop, v);
                }
            }
            "listen" => {
                let h = handle(arg(1)?)?;
                let on = arg(3)?.starts_with("on");
                let sig = arg(2)?.to_string();
                let n = self.node(h)?;
                if on {
                    n.listening.insert(sig);
                } else {
                    n.listening.remove(&sig);
                }
            }
            "observe" => {
                let h = handle(arg(1)?)?;
                let on = arg(3)? == "on";
                let prop = arg(2)?.to_string();
                let n = self.node(h)?;
                if on {
                    n.observing.insert(prop);
                } else {
                    n.observing.remove(&prop);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Parentless nodes (toplevels, and anything not inserted yet), in handle order.
    pub fn roots(&self) -> Vec<u64> {
        self.nodes.iter().filter(|(_, n)| n.parent.is_none()).map(|(h, _)| *h).collect()
    }

    /// Every node reachable from a root that is a window (any parentless node whose kind contains
    /// `window`), depth first, as view nodes.
    pub fn view(&self) -> Vec<ViewNode> {
        let mut out = Vec::new();
        for r in self.roots() {
            if self.nodes[&r].kind.contains("window") {
                self.walk(r, 0, &mut out);
            }
        }
        out
    }

    fn walk(&self, h: u64, depth: u32, out: &mut Vec<ViewNode>) {
        let n = &self.nodes[&h];
        out.push(ViewNode { depth, kind: n.kind.clone(), text: n.text().to_string() });
        for c in &n.children {
            self.walk(*c, depth + 1, out);
        }
    }

    /// Handles reachable from window roots, in view order.
    pub fn shown(&self) -> Vec<u64> {
        fn rec(m: &Mirror, h: u64, out: &mut Vec<u64>) {
            out.push(h);
            for c in &m.nodes[&h].children {
                rec(m, *c, out);
            }
        }
        let mut out = Vec::new();
        for r in self.roots() {
            if self.nodes[&r].kind.contains("window") {
                rec(self, r, &mut out);
            }
        }
        out
    }

    /// The one shown node `pred` accepts.
    pub fn find(&self, what: &str, pred: impl Fn(&Node) -> bool) -> Result<u64, String> {
        let hits: Vec<u64> = self.shown().into_iter().filter(|h| pred(&self.nodes[h])).collect();
        match hits.as_slice() {
            [h] => Ok(*h),
            [] => Err(format!("no {what}")),
            many => Err(format!("{} nodes match {what}: {many:?}", many.len())),
        }
    }

    /// The clickable node showing `text`: one that listens to `clicked`.
    pub fn button(&self, text: &str) -> Result<u64, String> {
        self.find(&format!("button showing {text:?}"), |n| n.text() == text && n.listening.contains("clicked"))
    }

    /// The text input named `field` (placeholder, hint text or accessible name).
    pub fn field(&self, field: &str) -> Result<u64, String> {
        self.find(&format!("text input named {field:?}"), |n| FIELD_PROPS.iter().any(|p| n.str_prop(p) == Some(field)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replays_a_counter_log() {
        let log = "create window #1\nset #1 title \"Counter\"\ncreate box #2\ncreate label #3\ninsert #2 #3 before=end slot=default\n\
                   create button #4\nset #4 label \"Increment\"\ninsert #2 #4 before=#3 slot=default\ninsert #1 #2 before=end slot=default\n\
                   set #3 label \"Count: 0\"\nlisten #4 clicked on\ncall #1 present []\nevent #4 clicked []\nset #3 label \"Count: 1\"\n";
        let m = Mirror::replay(log).unwrap();
        let texts: Vec<String> = m.view().into_iter().map(|n| n.text).collect();
        assert_eq!(texts, ["Counter", "", "Increment", "Count: 1"]);
        assert_eq!(m.button("Increment"), Ok(4));
        assert!(m.button("Count: 1").is_err(), "a label does not listen to clicked");
        let m = Mirror::replay(&format!("{log}remove #2 #4\ndestroy #4\n")).unwrap();
        assert_eq!(m.view().len(), 3);
    }
}
