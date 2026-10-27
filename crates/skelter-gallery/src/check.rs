//! `check`: every variant parses, compiles and renders one cell (its first args, `normal`, Dark
//! Modern). A compile error, a runtime error, a missing `subject` anchor when the story declares
//! states, or an empty render is a problem.

use crate::model::{Cell, State};
use crate::render::{Error, Renderer};
use serde::Serialize;
use std::path::PathBuf;

/// What a problem is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ProblemKind {
    /// The story.toml doesn't parse or is invalid.
    Story,
    /// The variant doesn't compile or throws while loading.
    Load,
    /// It threw while mounting or rendering, or reported an error.
    Runtime,
    /// The story declares states but the variant has no `anchor="subject"` node.
    NoSubject,
    /// It mounted nothing visible.
    Empty,
    /// No pixels (no GPU adapter).
    Render,
}

#[derive(Clone, Debug, Serialize)]
pub struct Problem {
    pub kind: ProblemKind,
    pub story: Option<String>,
    pub variant: Option<String>,
    pub file: Option<PathBuf>,
    pub message: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct CheckReport {
    pub stories: usize,
    pub variants: usize,
    pub problems: Vec<Problem>,
    /// Non-fatal notes (console warnings), as `story/variant: text`.
    pub warnings: Vec<String>,
}

impl CheckReport {
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
}

impl Renderer {
    /// Checks every variant of the catalog (see the module docs).
    pub fn check(&mut self) -> CheckReport {
        let stories = self.catalog().stories().to_vec();
        let mut rep = CheckReport { stories: stories.len(), ..CheckReport::default() };
        for s in &stories {
            for v in &s.variants {
                rep.variants += 1;
                let cell = Cell::new(s, v);
                let file = Some(s.path(&v.file));
                let mut problem = |kind, message: String| rep.problems.push(Problem { kind, story: Some(s.id.0.clone()), variant: Some(v.id.0.clone()), file: file.clone(), message });
                let m = match self.mount(&cell) {
                    Ok(m) => m,
                    Err(e) => {
                        let kind = match &e {
                            Error::Load { .. } => ProblemKind::Load,
                            Error::Render(_) => ProblemKind::Render,
                            _ => ProblemKind::Runtime,
                        };
                        problem(kind, e.to_string());
                        continue;
                    }
                };
                for w in &m.warnings {
                    if w.starts_with("error:") || w.starts_with("faulted:") || w.starts_with("runtime faulted:") || w.starts_with("console.error:") {
                        problem(ProblemKind::Runtime, w.clone());
                    }
                }
                rep.warnings.extend(m.warnings.iter().filter(|w| w.starts_with("console.warn") || w.starts_with("not settled")).map(|w| format!("{}/{}: {w}", s.id, v.id)));
                if s.states.iter().any(|st| *st != State::Normal) && !m.has_subject {
                    problem(ProblemKind::NoSubject, format!("the story shows states ({}) but no node has anchor=\"subject\"", s.states.iter().map(|s| s.name()).collect::<Vec<_>>().join(", ")));
                }
                let visible = self.backend().scene().read(|sc| m.top.iter().filter_map(|&h| sc.get(h)?.drawn()).any(|r| r.max.x > r.min.x && r.max.y > r.min.y));
                if !visible {
                    problem(ProblemKind::Empty, "the variant mounted nothing with a size".into());
                    continue;
                }
                if let Err(e) = self.harness().render() {
                    problem(ProblemKind::Render, e.to_string());
                }
            }
        }
        rep
    }
}
