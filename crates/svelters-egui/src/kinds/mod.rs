//! The kind table: every element, property, signal and method the egui backend knows.
//!
//! This is the single source of the egui vocabulary (plans/research/EGUI.md §6, "Rust kind table as
//! the single source"). `resolve`/`describe` answer from it, the scene stores one typed field per
//! [`P`], and [`crate::spec`] turns it into `metadata/semantics/egui.json` for the compiler, the
//! TestHost and the docs (checked by `tests/spec.rs`).
//!
//! Names follow N1 where egui has a type (`label`, `button`, `checkbox`, `text-edit`, `slider`:
//! kebab-case of `egui::Label` etc.) and GTK-familiar names for the kinds we build (`window`, `box`).
//! Layout properties are CSS names because taffy's are (D2, D3): they are setters in the style
//! table, so `<style>` can use them on egui.
//!
//! Files (ARCH-EMBED §9): this one holds the table types, the id scheme and [`KindRegistry`];
//! `builtin.rs` the built-in table [`KINDS`] with the `Widget` base and the leaves; `containers.rs`
//! the `window` and `box` tables. Writer lane: WIDGETS (one file per new family); RA1 adds provider
//! kinds to the registry.

mod builtin;
mod containers;

pub use builtin::{ALIGN, ALIGN_SELF, DIRECTION, JUSTIFY, KINDS, WRAP, WRAP_MODE};

/// A property's value type (also its ABI tag).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ty {
    Bool,
    F64,
    I64,
    Str,
    /// A string from a fixed list.
    Enum(&'static [&'static str]),
}

/// A property's default (JSON in the spec).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Def {
    Null,
    Bool(bool),
    F64(f64),
    I64(i64),
    Str(&'static str),
}

/// Every property the scene stores, by identity (one typed field each, see `scene/node.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum P {
    // Widget (every element inside a window)
    Enabled,
    Visible,
    Tooltip,
    AccessibleName,
    Width,
    Height,
    MinWidth,
    MinHeight,
    MaxWidth,
    MaxHeight,
    FlexGrow,
    FlexShrink,
    FlexBasis,
    AlignSelf,
    Margin,
    // Box (and the window's content area)
    FlexDirection,
    FlexWrap,
    Gap,
    RowGap,
    ColumnGap,
    Padding,
    AlignItems,
    JustifyContent,
    // Window
    Title,
    DefaultWidth,
    DefaultHeight,
    // Leaves
    Text,
    WrapMode,
    Selectable,
    Selected,
    Frame,
    Small,
    Checked,
    HintText,
    Multiline,
    Password,
    CharLimit,
    Value,
    Min,
    Max,
    Step,
    ShowValue,
}

/// How a property can change without the host setting it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Native {
    /// Only `set_prop` changes it. Observing it is still exact: the scene never changes it alone.
    Never,
    /// The widget changes it from user input during the pass (`prop_changed` when observed).
    Widget,
}

pub struct PropDef {
    pub name: &'static str,
    pub key: P,
    pub ty: Ty,
    pub default: Def,
    pub nullable: bool,
    pub native: Native,
    /// A layout property: a setter in the style table (D3), taffy reads it.
    pub layout: bool,
    pub doc: &'static str,
}

pub struct SignalDef {
    pub name: &'static str,
    pub args: &'static [(&'static str, Ty)],
    /// Delivered through `call_sync` before the pass; the handler's `true` stops the default.
    pub sync: bool,
    pub doc: &'static str,
}

pub struct MethodDef {
    pub name: &'static str,
    pub doc: &'static str,
}

pub struct KindDef {
    /// The kind's id in the spec (`kinds` keys) and in `resolve(SVL_KIND)`.
    pub id: &'static str,
    /// The element tag; `None` for abstract kinds.
    pub tag: Option<&'static str>,
    /// What it renders with.
    pub native: &'static str,
    pub base: Option<&'static str>,
    pub toplevel: bool,
    /// Accepts children (default slot `slot`).
    pub slot: Option<&'static str>,
    pub text_content: Option<&'static str>,
    pub present: Option<&'static str>,
    pub props: &'static [PropDef],
    pub signals: &'static [SignalDef],
    pub methods: &'static [MethodDef],
    pub doc: &'static str,
}

const fn prop(name: &'static str, key: P, ty: Ty, default: Def, doc: &'static str) -> PropDef {
    PropDef { name, key, ty, default, nullable: false, native: Native::Never, layout: false, doc }
}

const fn length(name: &'static str, key: P, doc: &'static str) -> PropDef {
    PropDef { name, key, ty: Ty::F64, default: Def::Null, nullable: true, native: Native::Never, layout: true, doc }
}

const fn layout(name: &'static str, key: P, ty: Ty, default: Def, doc: &'static str) -> PropDef {
    PropDef { name, key, ty, default, nullable: false, native: Native::Never, layout: true, doc }
}

const fn input(name: &'static str, key: P, ty: Ty, default: Def, doc: &'static str) -> PropDef {
    PropDef { name, key, ty, default, nullable: false, native: Native::Widget, layout: false, doc }
}

const fn signal(name: &'static str, args: &'static [(&'static str, Ty)], doc: &'static str) -> SignalDef {
    SignalDef { name, args, sync: false, doc }
}

/// The kinds the backend knows, with the id scheme over them (ARCH-EMBED §8.1).
///
/// EG0: a thin wrapper over the built-in table [`KINDS`] (kind ids `index + 1`, member ids
/// `(defining kind + 1) << 16 | (index + 1)`); the free functions of this module answer through
/// [`KindRegistry::global`]. RA1 builds it once per process from the built-ins plus the provider
/// kinds collected from `inventory` (pack-62 ids).
///
/// It reads [`KINDS`] itself rather than holding a copy of the slice: a copy made in a static
/// initializer is not guaranteed to point at the same definitions, and callers compare them by
/// address (`tests/spec.rs`).
pub struct KindRegistry {
    _builtins_only: (),
}

static REGISTRY: KindRegistry = KindRegistry { _builtins_only: () };

impl KindRegistry {
    /// The process's registry.
    pub fn global() -> &'static KindRegistry {
        &REGISTRY
    }

    /// Every kind, by index.
    pub fn kinds(&self) -> &'static [KindDef] {
        KINDS
    }

    /// Index of a kind by id.
    pub fn kind_index(&self, id: &str) -> Option<usize> {
        self.kinds().iter().position(|k| k.id == id)
    }

    /// Index of a kind by tag.
    pub fn tag_index(&self, tag: &str) -> Option<usize> {
        self.kinds().iter().position(|k| k.tag == Some(tag))
    }

    /// The kind and its bases, most derived first.
    pub fn chain(&self, kind: usize) -> Vec<usize> {
        let mut out = vec![kind];
        let mut k = &self.kinds()[kind];
        while let Some(b) = k.base {
            let i = self.kind_index(b).expect("base kind");
            out.push(i);
            k = &self.kinds()[i];
        }
        out
    }

    // ---- ids: (defining kind + 1) << 16 | (index + 1); kinds are index + 1 ----

    pub fn kind_id(&self, kind: usize) -> u32 {
        kind as u32 + 1
    }

    pub fn kind_of_id(&self, id: u32) -> Option<usize> {
        let i = (id as usize).checked_sub(1)?;
        (i < self.kinds().len()).then_some(i)
    }

    fn member_id(owner: usize, i: usize) -> u32 {
        ((owner as u32 + 1) << 16) | (i as u32 + 1)
    }

    fn split(&self, id: u32) -> Option<(usize, usize)> {
        let owner = ((id >> 16) as usize).checked_sub(1)?;
        let i = ((id & 0xffff) as usize).checked_sub(1)?;
        (owner < self.kinds().len()).then_some((owner, i))
    }

    /// A property of `kind` (or a base) by name: its id and definition.
    pub fn find_prop(&self, kind: usize, name: &str) -> Option<(u32, &'static PropDef)> {
        let kinds = self.kinds();
        self.chain(kind)
            .into_iter()
            .find_map(|k| kinds[k].props.iter().position(|p| p.name == name).map(|i| (Self::member_id(k, i), &kinds[k].props[i])))
    }

    /// The property definition behind an id, if `kind` has it.
    pub fn prop_def(&self, kind: usize, id: u32) -> Option<&'static PropDef> {
        let (owner, i) = self.split(id)?;
        let kinds = self.kinds();
        self.chain(kind).contains(&owner).then(|| kinds[owner].props.get(i)).flatten()
    }

    pub fn find_signal(&self, kind: usize, name: &str) -> Option<(u32, &'static SignalDef)> {
        let kinds = self.kinds();
        self.chain(kind)
            .into_iter()
            .find_map(|k| kinds[k].signals.iter().position(|s| s.name == name).map(|i| (Self::member_id(k, i), &kinds[k].signals[i])))
    }

    pub fn signal_def(&self, kind: usize, id: u32) -> Option<&'static SignalDef> {
        let (owner, i) = self.split(id)?;
        let kinds = self.kinds();
        self.chain(kind).contains(&owner).then(|| kinds[owner].signals.get(i)).flatten()
    }

    pub fn find_method(&self, kind: usize, name: &str) -> Option<(u32, &'static MethodDef)> {
        let kinds = self.kinds();
        self.chain(kind)
            .into_iter()
            .find_map(|k| kinds[k].methods.iter().position(|m| m.name == name).map(|i| (Self::member_id(k, i), &kinds[k].methods[i])))
    }

    pub fn method_def(&self, kind: usize, id: u32) -> Option<&'static MethodDef> {
        let (owner, i) = self.split(id)?;
        let kinds = self.kinds();
        self.chain(kind).contains(&owner).then(|| kinds[owner].methods.get(i)).flatten()
    }

    /// The default slot of a container kind (slot id 1; 0 is "default" on the ABI).
    pub fn slot_of(&self, kind: usize) -> Option<&'static str> {
        let kinds = self.kinds();
        self.chain(kind).into_iter().find_map(|k| kinds[k].slot)
    }

    /// The signal id of `name` on `kind`, or 0.
    pub fn signal_id(&self, kind: usize, name: &str) -> u32 {
        self.find_signal(kind, name).map(|(id, _)| id).unwrap_or(0)
    }

    /// The property id of `key` on `kind`, or 0.
    pub fn prop_id(&self, kind: usize, key: P) -> u32 {
        for k in self.chain(kind) {
            if let Some(i) = self.kinds()[k].props.iter().position(|p| p.key == key) {
                return Self::member_id(k, i);
            }
        }
        0
    }
}

// ---- the module's functions: the global registry's answers ----

/// Index of a kind in [`KINDS`] by id.
pub fn kind_index(id: &str) -> Option<usize> {
    KindRegistry::global().kind_index(id)
}

/// Index of a kind in [`KINDS`] by tag.
pub fn tag_index(tag: &str) -> Option<usize> {
    KindRegistry::global().tag_index(tag)
}

/// The kind and its bases, most derived first.
pub fn chain(kind: usize) -> Vec<usize> {
    KindRegistry::global().chain(kind)
}

pub fn kind_id(kind: usize) -> u32 {
    KindRegistry::global().kind_id(kind)
}

pub fn kind_of_id(id: u32) -> Option<usize> {
    KindRegistry::global().kind_of_id(id)
}

/// A property of `kind` (or a base) by name: its id and definition.
pub fn find_prop(kind: usize, name: &str) -> Option<(u32, &'static PropDef)> {
    KindRegistry::global().find_prop(kind, name)
}

/// The property definition behind an id, if `kind` has it.
pub fn prop_def(kind: usize, id: u32) -> Option<&'static PropDef> {
    KindRegistry::global().prop_def(kind, id)
}

pub fn find_signal(kind: usize, name: &str) -> Option<(u32, &'static SignalDef)> {
    KindRegistry::global().find_signal(kind, name)
}

pub fn signal_def(kind: usize, id: u32) -> Option<&'static SignalDef> {
    KindRegistry::global().signal_def(kind, id)
}

pub fn find_method(kind: usize, name: &str) -> Option<(u32, &'static MethodDef)> {
    KindRegistry::global().find_method(kind, name)
}

pub fn method_def(kind: usize, id: u32) -> Option<&'static MethodDef> {
    KindRegistry::global().method_def(kind, id)
}

/// The default slot of a container kind (slot id 1; 0 is "default" on the ABI).
pub fn slot_of(kind: usize) -> Option<&'static str> {
    KindRegistry::global().slot_of(kind)
}

/// The signal id of `name` on `kind`, or 0.
pub fn signal_id(kind: usize, name: &str) -> u32 {
    KindRegistry::global().signal_id(kind, name)
}

/// The property id of `key` on `kind`, or 0.
pub fn prop_id(kind: usize, key: P) -> u32 {
    KindRegistry::global().prop_id(kind, key)
}
