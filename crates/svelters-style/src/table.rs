//! A backend's style property table: which properties it has, how each is applied and the value
//! type the engine computes. The five toolkit tables are generated into [`crate::tables`].

/// How a toolkit styles natively.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    /// GTK CSS.
    Css,
    /// Qt style sheets.
    Qss,
    /// ST2's decl dialect (AppKit).
    Decl,
    /// No native styling: everything goes through setters (WinUI, egui).
    None,
}

/// How a property reaches the toolkit (CSS.md §2.4 item 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    /// The toolkit's own CSS engine applies the declaration; this engine orders rules but never
    /// decides the value, and leaves it out of [`ComputedStyle`](crate::ComputedStyle).
    Css,
    /// A widget property (`set_prop` of [`Prop::target`]).
    Setter,
    /// An attribute of the child on its parent (`set_child_attr` of [`Prop::target`]).
    ChildAttr,
}

/// The computed value type of a property.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueType {
    Length,
    Number,
    Int,
    Bool,
    String,
    /// One of [`Prop::values`].
    Enum,
    Color,
    /// One to four lengths, as CSS `margin`.
    Insets,
    FontWeight,
    /// `box-shadow`: `none` or a comma-separated list of shadows ([`crate::Shadow`]).
    Shadow,
    /// `transition`: `none` or a comma-separated list ([`crate::Transition`]).
    Transition,
    /// `background-image`: `none` or one `linear-gradient()` ([`crate::LinearGradient`]).
    Image,
    /// A value this engine keeps as text (`border`, untyped `box-shadow` and `via: css` values).
    Raw,
}

/// One property of a table.
#[derive(Clone, Copy, Debug)]
pub struct Prop {
    /// The style-sheet name: a standard CSS name where the mapping is faithful (CSS.md D1), the
    /// native name otherwise (`hexpand`).
    pub name: &'static str,
    pub via: Via,
    /// The native prop (setter) or child attribute (child-attr); empty for `via: css`.
    pub target: &'static str,
    pub ty: ValueType,
    /// The keywords of an [`ValueType::Enum`].
    pub values: &'static [&'static str],
    /// The tags it applies to (the parents' tags for a child attribute); empty = every tag.
    pub tags: &'static [&'static str],
    /// Only valid in `style=` / `style:` (no rule may set it).
    pub inline_only: bool,
}

impl Prop {
    /// Whether this engine computes the value (setters and child attributes).
    pub fn is_computed(&self) -> bool {
        self.via != Via::Css
    }

    /// Whether it applies to an element with `tag` (for a child attribute: whose parent has `tag`).
    pub fn applies_to(&self, tag: &str) -> bool {
        self.tags.is_empty() || self.tags.contains(&tag)
    }

    /// Whether CSS inherits it (CSS 2 / CSS Text / Fonts: the inherited properties a table can name).
    pub fn is_inherited(&self) -> bool {
        INHERITED.binary_search(&self.name).is_ok()
    }
}

/// The inherited standard properties, sorted.
const INHERITED: &[&str] = &[
    "color",
    "cursor",
    "direction",
    "font",
    "font-family",
    "font-size",
    "font-style",
    "font-variant",
    "font-weight",
    "letter-spacing",
    "line-height",
    "text-align",
    "text-indent",
    "text-transform",
    "visibility",
    "white-space",
    "word-spacing",
];

/// A backend's property table.
#[derive(Clone, Copy, Debug)]
pub struct Table {
    pub toolkit: &'static str,
    pub dialect: Dialect,
    /// The selector kinds the old setter matcher accepted (`type`, `class`, `id`); kept for the
    /// compiler's diagnostics until D9 retires it.
    pub setter_selectors: &'static [&'static str],
    /// Length units and their scale to the toolkit's unit (`px` = 1).
    pub units: &'static [(&'static str, f32)],
    /// Sorted by name.
    pub props: &'static [Prop],
}

impl Table {
    /// Whether this engine computes `prop` on this backend: setters and child attributes always,
    /// and `via: css` properties too when the toolkit has no CSS engine of its own (egui), since
    /// nobody else decides them there.
    pub fn computes(&self, prop: &Prop) -> bool {
        prop.is_computed() || self.dialect == Dialect::None
    }

    pub fn get(&self, name: &str) -> Option<&'static Prop> {
        self.index_of(name).map(|i| &self.props[i])
    }

    /// The property's index in [`Table::props`], which [`ComputedStyle`](crate::ComputedStyle) keys by.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.props.binary_search_by(|p| p.name.cmp(name)).ok()
    }

    pub fn unit_scale(&self, unit: &str) -> Option<f32> {
        self.units.iter().find(|(u, _)| u.eq_ignore_ascii_case(unit)).map(|(_, s)| *s)
    }

    /// The generated table of `toolkit` (`gtk`, `qt`, `appkit`, `winui`, `egui`).
    pub fn for_toolkit(toolkit: &str) -> Option<&'static Table> {
        crate::tables::ALL.iter().copied().find(|t| t.toolkit == toolkit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inherited_is_sorted() {
        assert!(INHERITED.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn generated_tables_are_sorted_and_typed() {
        for t in crate::tables::ALL {
            assert!(t.props.windows(2).all(|w| w[0].name < w[1].name), "{} props unsorted", t.toolkit);
            for p in t.props {
                assert_eq!(p.ty == ValueType::Enum, !p.values.is_empty(), "{}.{}", t.toolkit, p.name);
            }
        }
        let egui = Table::for_toolkit("egui").unwrap();
        assert_eq!(egui.get("flex-direction").unwrap().via, Via::Setter);
        assert_eq!(egui.unit_scale("px"), Some(1.0));
    }
}
