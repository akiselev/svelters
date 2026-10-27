//! The data model: stories, variants, states, themes and cells (the brief's shared contract).

use serde::{Serialize, Serializer};
use skelter_widgets::theme::{ColorScheme, Contrast, Density};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// The families a story may belong to (`story.toml`'s `family`). A host adds its own with
/// [`crate::Extensions::families`].
pub const FAMILIES: &[&str] = &["controls", "text", "pickers", "views", "menus", "tabs", "dialogs", "layout", "data", "feedback"];

/// A story's stable id (lowercase letters, digits and `-`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct StoryId(pub String);

/// A variant's stable id, unique within its story.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct VariantId(pub String);

macro_rules! id_impls {
    ($t:ident) => {
        impl $t {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
        impl From<&str> for $t {
            fn from(s: &str) -> $t {
                $t(s.to_string())
            }
        }
    };
}
id_impls!(StoryId);
id_impls!(VariantId);

/// Whether `s` is a valid id: lowercase ASCII letters, digits and `-`, starting with a letter.
pub fn is_id(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_lowercase()) && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') && !s.ends_with('-')
}

/// One story: a component (or a family of kinds) with its variants. Paths are relative to
/// [`Story::root`], the gallery root it was found in.
#[derive(Clone, Debug, Serialize)]
pub struct Story {
    pub id: StoryId,
    pub title: String,
    pub family: String,
    /// The element kinds this story covers (JS tags, as written in the variant files).
    pub kinds: Vec<String>,
    /// Each cell's logical size in points.
    pub size: [u32; 2],
    /// The states each variant is shown in (default [`State::DEFAULT_SET`]).
    pub states: Vec<State>,
    pub notes: Option<String>,
    pub variants: Vec<Variant>,
    /// The story's directory, relative to `root` (`stories/controls/button`).
    pub dir: PathBuf,
    /// The gallery root (absolute) the story was found under.
    #[serde(skip)]
    pub root: PathBuf,
}

impl Story {
    pub fn variant(&self, id: &str) -> Option<&Variant> {
        self.variants.iter().find(|v| v.id.0 == id)
    }

    /// `rel` (a path relative to the gallery root) as an absolute path.
    pub fn path(&self, rel: &Path) -> PathBuf {
        self.root.join(rel)
    }

    /// The story.toml file.
    pub fn manifest(&self) -> PathBuf {
        self.root.join(&self.dir).join("story.toml")
    }
}

/// One variant: a self-contained Svelte component.
#[derive(Clone, Debug, Serialize)]
pub struct Variant {
    pub id: VariantId,
    pub title: String,
    /// The component file, relative to the gallery root.
    pub file: PathBuf,
    /// The args matrix: each combination (keys in order, each key's values in the order given) is
    /// passed as props.
    pub args: BTreeMap<String, Vec<serde_json::Value>>,
    /// The props the component destructures from `$props()` (read from its source when found;
    /// best effort). `disabled` among them makes the `disabled` state a prop.
    pub props: Vec<String>,
}

impl Variant {
    /// Whether the `disabled` state is passed as the prop `disabled = true`.
    pub fn takes_disabled(&self) -> bool {
        self.props.iter().any(|p| p == "disabled")
    }

    /// Every args combination, in the stable order: keys in key order, the first key slowest.
    pub fn arg_combinations(&self) -> Vec<BTreeMap<String, serde_json::Value>> {
        let mut out = vec![BTreeMap::new()];
        for (k, vals) in &self.args {
            let mut next = Vec::with_capacity(out.len() * vals.len());
            for base in &out {
                for v in vals {
                    let mut m = base.clone();
                    m.insert(k.clone(), v.clone());
                    next.push(m);
                }
            }
            out = next;
        }
        out
    }
}

/// A state a cell is shown in. `Normal` forces nothing; `Disabled` is the prop `disabled = true`
/// when the variant takes one, else the forced `:disabled` bit; the others are forced state bits
/// on the `subject` anchors (`SkelterBackend::force_state`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum State {
    Normal,
    Hover,
    Active,
    Focus,
    FocusVisible,
    Checked,
    Disabled,
}

impl State {
    pub const ALL: [State; 7] = [State::Normal, State::Hover, State::Active, State::Focus, State::FocusVisible, State::Checked, State::Disabled];
    /// The states a story shows when its story.toml names none.
    pub const DEFAULT_SET: [State; 5] = [State::Normal, State::Hover, State::Active, State::FocusVisible, State::Disabled];

    pub fn name(self) -> &'static str {
        match self {
            State::Normal => "normal",
            State::Hover => "hover",
            State::Active => "active",
            State::Focus => "focus",
            State::FocusVisible => "focus-visible",
            State::Checked => "checked",
            State::Disabled => "disabled",
        }
    }

    pub fn parse(s: &str) -> Option<State> {
        State::ALL.into_iter().find(|st| st.name() == s)
    }

    /// The pseudo-class whose bits are forced for it (`None` for `normal`).
    pub fn pseudo_class(self) -> Option<&'static str> {
        (self != State::Normal).then(|| self.name())
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl Serialize for State {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}

/// A theme axis value: one of VS Code's four sets, or a token set a host added
/// ([`crate::Extensions::themes`]).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Theme {
    DarkModern,
    LightModern,
    HcDark,
    HcLight,
    /// A host's token set, by name.
    Extra(String),
}

impl Theme {
    pub const BUILTIN: [Theme; 4] = [Theme::DarkModern, Theme::LightModern, Theme::HcDark, Theme::HcLight];

    pub fn name(&self) -> &str {
        match self {
            Theme::DarkModern => "dark-modern",
            Theme::LightModern => "light-modern",
            Theme::HcDark => "hc-dark",
            Theme::HcLight => "hc-light",
            Theme::Extra(n) => n,
        }
    }

    /// One of the four built-in names; a host's names are resolved by [`crate::Extensions`].
    pub fn builtin(name: &str) -> Option<Theme> {
        Theme::BUILTIN.into_iter().find(|t| t.name() == name)
    }

    /// The built-in set's scheme and contrast (`None` for a host's set).
    pub fn recipe(&self) -> Option<(ColorScheme, Contrast)> {
        match self {
            Theme::DarkModern => Some((ColorScheme::Dark, Contrast::Normal)),
            Theme::LightModern => Some((ColorScheme::Light, Contrast::Normal)),
            Theme::HcDark => Some((ColorScheme::Dark, Contrast::High)),
            Theme::HcLight => Some((ColorScheme::Light, Contrast::High)),
            Theme::Extra(_) => None,
        }
    }
}

impl fmt::Display for Theme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl Serialize for Theme {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}

/// A density by its data name (`compact`, `default`, `comfortable`).
pub fn density_named(name: &str) -> Option<Density> {
    Density::ALL.into_iter().find(|d| d.name() == name)
}

/// One rendered cell: (story, variant, args combination, state, theme, density, zoom).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Cell {
    pub story: StoryId,
    pub variant: VariantId,
    pub args: BTreeMap<String, serde_json::Value>,
    pub state: State,
    pub theme: Theme,
    #[serde(serialize_with = "ser_density")]
    pub density: Density,
    pub zoom: f32,
}

fn ser_density<S: Serializer>(d: &Density, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(d.name())
}

impl Cell {
    /// The cell of `story`/`variant` in the defaults: no args (or each arg's first value), normal,
    /// Dark Modern, the default density, zoom 1.
    pub fn new(story: &Story, variant: &Variant) -> Cell {
        let args = variant.args.iter().filter_map(|(k, v)| Some((k.clone(), v.first()?.clone()))).collect();
        Cell { story: story.id.clone(), variant: variant.id.clone(), args, state: State::Normal, theme: Theme::DarkModern, density: Density::Default, zoom: 1.0 }
    }

    /// The args as `k=v` pairs (`size=small,tone=primary`), empty when none.
    pub fn args_label(&self) -> String {
        self.args.iter().map(|(k, v)| format!("{k}={}", value_label(v))).collect::<Vec<_>>().join(",")
    }

    /// A short label: `variant [args] state theme [density] [zoom]`, the axes at their defaults left out
    /// except state and theme.
    pub fn label(&self) -> String {
        let mut s = self.variant.0.clone();
        let a = self.args_label();
        if !a.is_empty() {
            s.push_str(&format!(" {a}"));
        }
        s.push_str(&format!(" {} {}", self.state, self.theme));
        if self.density != Density::Default {
            s.push_str(&format!(" {}", self.density.name()));
        }
        if self.zoom != 1.0 {
            s.push_str(&format!(" x{}", self.zoom));
        }
        s
    }
}

/// A JSON value as a label: strings bare, everything else as JSON.
pub fn value_label(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        v => v.to_string(),
    }
}

/// A `k=v` value from the command line: JSON when it parses (`3`, `true`, `"x"`), else a string.
pub fn parse_arg_value(s: &str) -> serde_json::Value {
    serde_json::from_str(s).unwrap_or_else(|_| serde_json::Value::String(s.to_string()))
}
