//! `story.toml` parsing and discovery: every story under the gallery roots, with errors that name
//! the file and the field.

use crate::model::{FAMILIES, State, Story, StoryId, Variant, VariantId, is_id};
use serde::Serialize;
use skelter_core::KindRegistry;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

/// A story that could not be read: the file, the field (`variant[1].file`) when one is at fault,
/// and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StoryError {
    pub file: PathBuf,
    pub field: Option<String>,
    pub message: String,
}

impl StoryError {
    fn new(file: &Path, field: Option<&str>, message: impl Into<String>) -> StoryError {
        StoryError { file: file.to_path_buf(), field: field.map(str::to_string), message: message.into() }
    }
}

impl fmt::Display for StoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.field {
            Some(field) => write!(f, "{}: {field}: {}", self.file.display(), self.message),
            None => write!(f, "{}: {}", self.file.display(), self.message),
        }
    }
}

impl std::error::Error for StoryError {}

/// Every story found, in (family, id) order.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    stories: Vec<Story>,
}

/// What the stories cover of a registry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Coverage {
    /// Registered kinds no story names, sorted.
    pub missing: Vec<String>,
    /// (story, kind): story kinds the registry lacks.
    pub unknown: Vec<(String, String)>,
    /// Registered kinds some story names, sorted.
    pub covered: Vec<String>,
}

/// Every story under `roots` (each a gallery root: `stories/<family>/<id>/story.toml`), with the
/// built-in families. All errors are returned, each naming its file.
pub fn discover(roots: &[PathBuf]) -> Result<Catalog, Vec<StoryError>> {
    discover_with(roots, &[])
}

/// [`discover`] accepting `families` on top of [`FAMILIES`].
pub fn discover_with(roots: &[PathBuf], families: &[String]) -> Result<Catalog, Vec<StoryError>> {
    let mut stories: Vec<Story> = Vec::new();
    let mut errors = Vec::new();
    for root in roots {
        let root = match crate::model::canonicalize(root) {
            Ok(r) => r,
            Err(e) => {
                errors.push(StoryError::new(root, None, format!("gallery root: {e}")));
                continue;
            }
        };
        let stories_dir = root.join("stories");
        let Ok(fams) = std::fs::read_dir(&stories_dir) else {
            errors.push(StoryError::new(&stories_dir, None, "no stories directory"));
            continue;
        };
        let mut manifests: Vec<PathBuf> = Vec::new();
        for fam in fams.flatten() {
            let Ok(ids) = std::fs::read_dir(fam.path()) else { continue };
            for id in ids.flatten() {
                let m = id.path().join("story.toml");
                if m.is_file() {
                    manifests.push(m);
                } else if id.path().is_dir() {
                    errors.push(StoryError::new(&id.path(), None, "a story directory without a story.toml"));
                }
            }
        }
        manifests.sort();
        for m in manifests {
            match parse_story(&root, &m, families) {
                Ok(s) => stories.push(s),
                Err(e) => errors.extend(e),
            }
        }
    }
    let mut seen: BTreeMap<&StoryId, &Path> = BTreeMap::new();
    for s in &stories {
        if let Some(first) = seen.insert(&s.id, &s.root) {
            errors.push(StoryError::new(&s.manifest(), Some("id"), format!("story {:?} is also defined under {}", s.id.0, first.display())));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    stories.sort_by(|a, b| (&a.family, &a.id).cmp(&(&b.family, &b.id)));
    Ok(Catalog { stories })
}

impl Catalog {
    /// A catalog of these stories (tests, hosts that build stories in code).
    pub fn from_stories(mut stories: Vec<Story>) -> Catalog {
        stories.sort_by(|a, b| (&a.family, &a.id).cmp(&(&b.family, &b.id)));
        Catalog { stories }
    }

    pub fn stories(&self) -> &[Story] {
        &self.stories
    }

    pub fn get(&self, id: &str) -> Option<&Story> {
        self.stories.iter().find(|s| s.id.0 == id)
    }

    /// The stories of `family`.
    pub fn family<'a>(&'a self, family: &'a str) -> impl Iterator<Item = &'a Story> + 'a {
        self.stories.iter().filter(move |s| s.family == family)
    }

    /// Registered kinds with no story, and story kinds the registry lacks. A story kind is
    /// looked up under its core name (`combo-box` is skelter-core's `select`).
    pub fn coverage(&self, registry: &KindRegistry) -> Coverage {
        let registered: BTreeSet<String> = registry.iter().map(|e| e.spec.name.to_string()).collect();
        let mut covered = BTreeSet::new();
        let mut unknown = Vec::new();
        for s in &self.stories {
            for k in &s.kinds {
                let core = svelters_skelter::kinds::core_tag(k);
                if registered.contains(core) {
                    covered.insert(core.to_string());
                } else {
                    unknown.push((s.id.0.clone(), k.clone()));
                }
            }
        }
        let missing = registered.difference(&covered).cloned().collect();
        Coverage { missing, unknown, covered: covered.into_iter().collect() }
    }
}

// ---- story.toml ----

const STORY_KEYS: &[&str] = &["id", "title", "family", "kinds", "size", "states", "notes", "variant"];
const VARIANT_KEYS: &[&str] = &["id", "title", "file", "args"];

/// Parses `manifest` (a `story.toml` under `root`). Every problem found is returned.
pub fn parse_story(root: &Path, manifest: &Path, families: &[String]) -> Result<Story, Vec<StoryError>> {
    let text = std::fs::read_to_string(manifest).map_err(|e| vec![StoryError::new(manifest, None, e.to_string())])?;
    parse_story_text(root, manifest, &text, families)
}

/// [`parse_story`] of `text` (the file's contents, or an editor's buffer).
pub fn parse_story_text(root: &Path, manifest: &Path, text: &str, families: &[String]) -> Result<Story, Vec<StoryError>> {
    let table: toml::Table = text.parse().map_err(|e: toml::de::Error| vec![StoryError::new(manifest, None, format!("not TOML: {}", e.message()))])?;
    let mut errs = Vec::new();
    let mut err = |field: &str, msg: String| errs.push(StoryError::new(manifest, Some(field), msg));
    for k in table.keys() {
        if !STORY_KEYS.contains(&k.as_str()) {
            err(k, format!("unknown field (expected one of {})", STORY_KEYS.join(", ")));
        }
    }
    let dir = manifest.parent().unwrap_or(root);
    let rel_dir = dir.strip_prefix(root).unwrap_or(dir).to_path_buf();

    let string = |t: &toml::Table, k: &str, required: bool, err: &mut dyn FnMut(&str, String)| -> Option<String> {
        match t.get(k) {
            Some(toml::Value::String(s)) if !s.trim().is_empty() => Some(s.clone()),
            Some(toml::Value::String(_)) => {
                err(k, "is empty".into());
                None
            }
            Some(v) => {
                err(k, format!("must be a string, not {}", v.type_str()));
                None
            }
            None if required => {
                err(k, "is missing".into());
                None
            }
            None => None,
        }
    };

    let id = string(&table, "id", true, &mut err);
    if let Some(id) = &id {
        if !is_id(id) {
            err("id", format!("{id:?} is not an id (lowercase letters, digits and `-`)"));
        } else if dir.file_name().and_then(|n| n.to_str()) != Some(id.as_str()) {
            err("id", format!("{id:?} differs from its directory name {:?}", dir.file_name().unwrap_or_default()));
        }
    }
    let title = string(&table, "title", true, &mut err);
    let family = string(&table, "family", true, &mut err);
    if let Some(f) = &family {
        if !FAMILIES.contains(&f.as_str()) && !families.iter().any(|x| x == f) {
            err("family", format!("{f:?} is not a family ({})", FAMILIES.join(", ")));
        } else if dir.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()) != Some(f.as_str()) {
            err("family", format!("{f:?} differs from its parent directory's name"));
        }
    }
    let notes = string(&table, "notes", false, &mut err);
    let mut kinds = Vec::new();
    match table.get("kinds") {
        Some(toml::Value::Array(a)) if !a.is_empty() => {
            for (i, v) in a.iter().enumerate() {
                match v.as_str() {
                    Some(s) if !s.is_empty() => kinds.push(s.to_string()),
                    _ => err(&format!("kinds[{i}]"), "must be a kind name".into()),
                }
            }
        }
        Some(toml::Value::Array(_)) => err("kinds", "is empty: name the kinds the story covers".into()),
        Some(v) => err("kinds", format!("must be an array of kind names, not {}", v.type_str())),
        None => err("kinds", "is missing".into()),
    }
    let mut size = [0u32; 2];
    match table.get("size") {
        Some(toml::Value::Array(a)) if a.len() == 2 => {
            for (i, v) in a.iter().enumerate() {
                match v.as_integer() {
                    Some(n) if (1..=8192).contains(&n) => size[i] = n as u32,
                    _ => err(&format!("size[{i}]"), "must be an integer from 1 to 8192 (points)".into()),
                }
            }
        }
        Some(_) => err("size", "must be [width, height] in points".into()),
        None => err("size", "is missing".into()),
    }
    let mut states = Vec::new();
    match table.get("states") {
        None => states.extend(State::DEFAULT_SET),
        Some(toml::Value::Array(a)) if !a.is_empty() => {
            for (i, v) in a.iter().enumerate() {
                match v.as_str().and_then(State::parse) {
                    Some(s) if states.contains(&s) => err(&format!("states[{i}]"), format!("{s} is listed twice")),
                    Some(s) => states.push(s),
                    None => err(&format!("states[{i}]"), format!("{v} is not a state ({})", State::ALL.map(State::name).join(", "))),
                }
            }
        }
        Some(_) => err("states", "must be a non-empty array of state names".into()),
    }

    let mut variants: Vec<Variant> = Vec::new();
    match table.get("variant") {
        Some(toml::Value::Array(a)) if !a.is_empty() => {
            for (i, v) in a.iter().enumerate() {
                let at = format!("variant[{i}]");
                let Some(t) = v.as_table() else {
                    err(&at, "must be a table ([[variant]])".into());
                    continue;
                };
                let mut verr = |f: &str, m: String| err(&format!("{at}.{f}"), m);
                for k in t.keys() {
                    if !VARIANT_KEYS.contains(&k.as_str()) {
                        verr(k, format!("unknown field (expected one of {})", VARIANT_KEYS.join(", ")));
                    }
                }
                let vid = string(t, "id", true, &mut verr);
                if let Some(vid) = &vid {
                    if !is_id(vid) {
                        verr("id", format!("{vid:?} is not an id (lowercase letters, digits and `-`)"));
                    } else if variants.iter().any(|x| x.id.0 == *vid) {
                        verr("id", format!("{vid:?} is used by another variant"));
                    }
                }
                let vtitle = string(t, "title", true, &mut verr);
                let file = string(t, "file", true, &mut verr);
                let mut props = Vec::new();
                if let Some(f) = &file {
                    let p = dir.join(f);
                    if Path::new(f).is_absolute() || f.contains("..") || !f.ends_with(".svelte") {
                        verr("file", format!("{f:?} must be a .svelte file in the story's directory"));
                    } else {
                        match std::fs::read_to_string(&p) {
                            Ok(src) => props = declared_props(&src),
                            Err(e) => verr("file", format!("{}: {e}", p.display())),
                        }
                    }
                }
                let mut args = BTreeMap::new();
                match t.get("args") {
                    None => {}
                    Some(toml::Value::Table(at)) => {
                        for (k, vals) in at {
                            let Some(arr) = vals.as_array().filter(|a| !a.is_empty()) else {
                                verr(&format!("args.{k}"), "must be a non-empty array of values".into());
                                continue;
                            };
                            if k == "disabled" {
                                verr(&format!("args.{k}"), "`disabled` is a state, not an arg (list it in `states`)".into());
                                continue;
                            }
                            match arr.iter().map(to_json).collect::<Result<Vec<_>, _>>() {
                                Ok(vs) => {
                                    args.insert(k.clone(), vs);
                                }
                                Err(m) => verr(&format!("args.{k}"), m),
                            }
                        }
                    }
                    Some(v) => verr("args", format!("must be a table of arrays, not {}", v.type_str())),
                }
                if let (Some(id), Some(title), Some(file)) = (vid, vtitle, file) {
                    variants.push(Variant { id: VariantId(id), title, file: rel_dir.join(file), args, props });
                }
            }
        }
        Some(toml::Value::Array(_)) | None => err("variant", "a story needs at least one [[variant]]".into()),
        Some(v) => err("variant", format!("must be an array of tables ([[variant]]), not {}", v.type_str())),
    }

    if !errs.is_empty() {
        return Err(errs);
    }
    Ok(Story {
        id: StoryId(id.expect("checked")),
        title: title.expect("checked"),
        family: family.expect("checked"),
        kinds,
        size,
        states,
        notes,
        variants,
        dir: rel_dir,
        root: root.to_path_buf(),
    })
}

fn to_json(v: &toml::Value) -> Result<serde_json::Value, String> {
    Ok(match v {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_json::Value::from(*i),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f).map(serde_json::Value::Number).ok_or("a float that isn't finite")?,
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Array(a) => serde_json::Value::Array(a.iter().map(to_json).collect::<Result<_, _>>()?),
        toml::Value::Table(t) => serde_json::Value::Object(t.iter().map(|(k, v)| Ok((k.clone(), to_json(v)?))).collect::<Result<_, String>>()?),
        toml::Value::Datetime(_) => return Err("dates are not prop values".into()),
    })
}

/// The names a component destructures from `$props()`: `let { a, b = 1, c: d, ...rest } = $props()`
/// gives `a`, `b`, `c`. Best effort over the source text (no compiler): empty when there is no
/// such destructuring.
pub fn declared_props(src: &str) -> Vec<String> {
    let Some(at) = src.find("$props()") else { return Vec::new() };
    let before = src[..at].trim_end();
    let Some(before) = before.strip_suffix('=') else { return Vec::new() };
    let before = before.trim_end();
    // a type annotation (`}: Props =`) sits between the pattern and `=`
    let end = match before.rfind('}') {
        Some(e) => e,
        None => return Vec::new(),
    };
    // the matching `{`
    let bytes = before.as_bytes();
    let mut depth = 0i32;
    let mut start = None;
    for i in (0..=end).rev() {
        match bytes[i] {
            b'}' | b']' | b')' => depth += 1,
            b'{' | b'[' | b'(' => {
                depth -= 1;
                if depth == 0 {
                    start = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(start) = start else { return Vec::new() };
    let body = &before[start + 1..end];
    // split at top-level commas
    let mut out = Vec::new();
    let (mut depth, mut from) = (0i32, 0usize);
    let mut push = |part: &str| {
        let p = part.trim();
        if p.is_empty() || p.starts_with("...") {
            return;
        }
        let name: String = p.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$').collect();
        if !name.is_empty() {
            out.push(name);
        }
    };
    for (i, c) in body.char_indices() {
        match c {
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                push(&body[from..i]);
                from = i + 1;
            }
            _ => {}
        }
    }
    push(&body[from..]);
    out
}
