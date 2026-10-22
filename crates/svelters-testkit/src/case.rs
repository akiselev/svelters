//! The case file: a `.svelte` component with a header comment and an `@test` comment.
//!
//! ```svelte
//! <!--
//! @case A click reaches JS and JS updates the label.
//! @toolkits gtk qt egui
//! @xfail qt "why it fails there"
//! -->
//! <script>
//!   import { Window, Column, Button } from 'svelters:testkit';
//!   let count = $state(0);
//! </script>
//!
//! <Window title="Counter">
//!   <Column>
//!     <label>Count: {count}</label>
//!     <Button text="Increment" onclick={() => count++} />
//!   </Column>
//! </Window>
//!
//! <!--
//! @test
//! expect "Count: 0"
//! click "Increment"
//! expect "Count: 1"
//! -->
//! ```
//!
//! Header keys (any comment before `@test`, one per line): `@case <one-line description>`,
//! `@toolkits <names>` (default: gtk qt egui), `@xfail <backend> "<reason>"` (repeatable).
//!
//! Steps (one per line in the `@test` comment; `#` starts a comment line; strings are JSON strings):
//!
//! | Step | Meaning |
//! |---|---|
//! | `expect "<text>"` | some node shows exactly `<text>` (label, button text, entry text, window title) |
//! | `expect no "<text>"` | no node shows `<text>` |
//! | `expect order "<a>" "<b>" ...` | each text is shown, in this order in tree order |
//! | `click "<text>"` | click the button showing `<text>` |
//! | `type "<field>" "<text>"` | replace the content of the text input whose placeholder is `<field>` |
//! | `wait <ms>` | let `<ms>` of time pass (timers, transitions) before the next step |
//! | `snapshot "<name>"` | egui with the `snapshots` feature: compare a rendering with `snapshots/<case>-<name>.png` |
//!
//! Expectations are checked on the settled state: every backend runs until the host is idle
//! before the expectations that follow an action.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub enum Expect {
    Text(String),
    NoText(String),
    Order(Vec<String>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Expect(Expect),
    Click(String),
    Type { field: String, text: String },
    Wait(u64),
    Snapshot(String),
}

#[derive(Clone, Debug)]
pub struct Case {
    /// The file stem.
    pub name: String,
    pub path: PathBuf,
    pub about: String,
    pub toolkits: Vec<String>,
    /// `(backend, reason)`: the case is expected to fail there.
    pub xfail: Vec<(String, String)>,
    pub steps: Vec<Step>,
    /// 1-based line of each step in the file, for messages.
    pub lines: Vec<usize>,
}

/// The toolkits of the kit, and the default `@toolkits`.
pub const TOOLKITS: [&str; 3] = ["gtk", "qt", "egui"];

impl Case {
    pub fn targets(&self, toolkit: &str) -> bool {
        self.toolkits.iter().any(|t| t == toolkit)
    }

    /// The `@xfail` reason for `backend`, if any.
    pub fn xfail(&self, backend: &str) -> Option<&str> {
        self.xfail.iter().find(|(b, _)| b == backend).map(|(_, r)| r.as_str())
    }

    /// `<dir>/<toolkit>/<case>/dist/app.js`, where `svelters-cases` puts the bundle.
    pub fn bundle(&self, out: &Path, toolkit: &str) -> PathBuf {
        out.join(toolkit).join(&self.name).join("dist/app.js")
    }
}

/// Every case (`*.svelte`, not `_`-prefixed) in `dir`, sorted by name.
pub fn load_dir(dir: &Path) -> Result<Vec<Case>, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "svelte") && !p.file_name().unwrap().to_string_lossy().starts_with('_'))
        .collect();
    files.sort();
    files.iter().map(|p| load(p)).collect()
}

pub fn load(path: &Path) -> Result<Case, String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = path.file_stem().unwrap().to_string_lossy().into_owned();
    let mut case = parse(&name, &src).map_err(|e| format!("{}:{e}", path.display()))?;
    case.path = path.to_path_buf();
    Ok(case)
}

/// Parses a case's source. Errors are `<line>: <message>`.
pub fn parse(name: &str, src: &str) -> Result<Case, String> {
    let mut case = Case {
        name: name.to_string(),
        path: PathBuf::new(),
        about: String::new(),
        toolkits: TOOLKITS.iter().map(|s| s.to_string()).collect(),
        xfail: Vec::new(),
        steps: Vec::new(),
        lines: Vec::new(),
    };
    let mut in_comment = false;
    let mut in_test = false;
    let mut saw_test = false;
    for (i, raw) in src.lines().enumerate() {
        let n = i + 1;
        let mut line = raw.trim();
        if !in_comment {
            match line.strip_prefix("<!--") {
                Some(rest) => {
                    in_comment = true;
                    line = rest.trim();
                }
                None => continue,
            }
        }
        let closes = line.ends_with("-->");
        if closes {
            line = line[..line.len() - 3].trim();
        }
        if in_test {
            if !line.is_empty() && !line.starts_with('#') {
                case.steps.push(step(line).map_err(|e| format!("{n}: {e}"))?);
                case.lines.push(n);
            }
        } else if line == "@test" {
            if saw_test {
                return Err(format!("{n}: a second @test block"));
            }
            in_test = true;
            saw_test = true;
        } else if let Some(rest) = line.strip_prefix("@case") {
            case.about = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("@toolkits") {
            case.toolkits = rest.split([' ', ',']).filter(|s| !s.is_empty()).map(str::to_string).collect();
        } else if let Some(rest) = line.strip_prefix("@xfail") {
            let mut w = Words::new(rest);
            let backend = w.word().ok_or_else(|| format!("{n}: @xfail needs a backend"))?;
            let reason = w.string().map_err(|e| format!("{n}: @xfail: {e}"))?;
            case.xfail.push((backend, reason));
        } else if line.starts_with('@') {
            return Err(format!("{n}: unknown header key {line}"));
        }
        if closes {
            in_comment = false;
            in_test = false;
        }
    }
    if !saw_test {
        return Err("0: no @test comment".into());
    }
    Ok(case)
}

fn step(line: &str) -> Result<Step, String> {
    let mut w = Words::new(line);
    let verb = w.word().unwrap_or_default();
    let s = match verb.as_str() {
        "expect" => match w.peek_word().as_deref() {
            Some("no") => {
                w.word();
                Step::Expect(Expect::NoText(w.string()?))
            }
            Some("order") => {
                w.word();
                let mut texts = Vec::new();
                while !w.at_end() {
                    texts.push(w.string()?);
                }
                if texts.len() < 2 {
                    return Err("expect order needs at least two texts".into());
                }
                Step::Expect(Expect::Order(texts))
            }
            _ => Step::Expect(Expect::Text(w.string()?)),
        },
        "click" => Step::Click(w.string()?),
        "type" => {
            let field = w.string()?;
            Step::Type { field, text: w.string()? }
        }
        "wait" => Step::Wait(w.word().and_then(|s| s.parse().ok()).ok_or("wait needs milliseconds")?),
        "snapshot" => {
            let name = w.string()?;
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                return Err(format!("snapshot name {name:?}: use [A-Za-z0-9_-]"));
            }
            Step::Snapshot(name)
        }
        other => return Err(format!("unknown step '{other}'")),
    };
    if !w.at_end() {
        return Err(format!("trailing text: {}", w.rest()));
    }
    Ok(s)
}

/// Bare words and JSON strings.
struct Words<'a> {
    s: &'a str,
}

impl<'a> Words<'a> {
    fn new(s: &'a str) -> Self {
        Words { s: s.trim_start() }
    }

    fn at_end(&self) -> bool {
        self.s.trim().is_empty()
    }

    fn rest(&self) -> &str {
        self.s.trim()
    }

    fn peek_word(&self) -> Option<String> {
        if self.s.starts_with('"') {
            return None;
        }
        self.s.split_whitespace().next().map(str::to_string)
    }

    fn word(&mut self) -> Option<String> {
        let w = self.peek_word()?;
        self.s = self.s[w.len()..].trim_start();
        Some(w)
    }

    fn string(&mut self) -> Result<String, String> {
        if !self.s.starts_with('"') {
            return Err(format!("expected a \"string\" at: {}", self.s.trim()));
        }
        let mut escaped = false;
        let end = self.s[1..]
            .char_indices()
            .find(|&(_, c)| {
                let hit = c == '"' && !escaped;
                escaped = c == '\\' && !escaped;
                hit
            })
            .map(|(i, _)| i + 2)
            .ok_or("unterminated string")?;
        let v: String = serde_json::from_str(&self.s[..end]).map_err(|e| e.to_string())?;
        self.s = self.s[end..].trim_start();
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"<!--
@case Counter.
@toolkits gtk egui
@xfail gtk "no reason"
-->
<script>let n = $state(0);</script>
<!-- a plain comment -->
<label>{n}</label>
<!--
@test
# comment
expect "Count: 0"
click "Inc \"x\""
type "Name" "Ann"
expect no "Count: 1"
expect order "a" "b"
wait 200
snapshot "after-1"
-->
"#;

    #[test]
    fn parses_header_and_steps() {
        let c = parse("counter", SRC).unwrap();
        assert_eq!(c.about, "Counter.");
        assert_eq!(c.toolkits, ["gtk", "egui"]);
        assert_eq!(c.xfail("gtk"), Some("no reason"));
        assert_eq!(
            c.steps,
            [
                Step::Expect(Expect::Text("Count: 0".into())),
                Step::Click("Inc \"x\"".into()),
                Step::Type { field: "Name".into(), text: "Ann".into() },
                Step::Expect(Expect::NoText("Count: 1".into())),
                Step::Expect(Expect::Order(vec!["a".into(), "b".into()])),
                Step::Wait(200),
                Step::Snapshot("after-1".into()),
            ]
        );
        assert_eq!(c.lines[0], 12);
    }

    #[test]
    fn rejects_bad_steps() {
        let bad = |t: &str| parse("x", &format!("<!--\n@test\n{t}\n-->")).unwrap_err();
        assert!(bad("press \"x\"").contains("unknown step"));
        assert!(bad("expect Count").contains("expected a \"string\""));
        assert!(bad("click \"a\" \"b\"").contains("trailing"));
        assert!(parse("x", "<label/>").unwrap_err().contains("no @test"));
    }
}
