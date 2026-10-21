//! Shapes of the generated tables (`generated/names.rs`) and name resolution over them.
//!
//! Platform-neutral: compiled and unit-tested on every target, so the tables and the lookup rules
//! are checked on Linux CI even though the backend itself only runs on Windows.

/// Wire conversion of a property or argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Bool,
    I32,
    F64,
    Str,
    /// Index into [`crate::names::ENUMS`]. Names in, kebab-case names out.
    Enum(u16),
    /// `Microsoft.UI.Xaml.Thickness`: a number, `"l,t,r,b"` / `"h,v"` / `"u"`, or a record.
    Thickness,
    /// `IReference<bool>`: null = indeterminate.
    NullableBool,
    /// An `object` property used for text: a boxed HSTRING.
    ObjectString,
    /// A hand-written converter in `custom.rs`.
    Custom,
}

pub struct Kind {
    pub tag: &'static str,
    pub native: &'static str,
    /// Indices into `CLASSES`, the class itself first, then its bases nearest first.
    pub chain: &'static [u16],
    pub toplevel: bool,
    /// The prop (JS name) that Text children of the element set.
    pub text_content: Option<&'static str>,
    /// Slot id (index + 1) of the default slot, 0 when the kind takes no children.
    pub default_slot: u32,
}

pub struct Prop {
    pub name: &'static str,
    pub native: &'static str,
    pub class: u16,
    pub ty: Ty,
    pub read: bool,
    pub write: bool,
}

pub struct Event {
    pub name: &'static str,
    pub native: &'static str,
    pub class: u16,
    /// Delivered through `call_sync` while the native handler runs.
    pub sync: bool,
    /// The ABI v2 window close rule (veto + post while listened).
    pub close: bool,
    /// `notify::<prop>`: a DependencyProperty change callback (`RegisterPropertyChangedCallback`) on the
    /// prop `native`; no payload, the listener reads the prop.
    pub notify: bool,
    pub payload: &'static [&'static str],
}

pub struct Slot {
    pub name: &'static str,
    pub native: &'static str,
    pub class: u16,
    pub many: bool,
    pub default: bool,
}

pub struct Attr {
    pub name: &'static str,
    pub native: &'static str,
    /// The class whose static `Set<native>` applies it (`Grid`).
    pub owner: u16,
    /// Slot id (index + 1) the attribute belongs to.
    pub slot: u32,
    pub default: i64,
}

pub struct Method {
    pub name: &'static str,
    pub native: &'static str,
    pub class: u16,
    pub params: &'static [(&'static str, Ty)],
    pub ret: Option<Ty>,
}

pub struct Enum {
    pub native: &'static str,
    pub members: &'static [(&'static str, i32)],
}

/// The observe class of a prop (`metadata/observe/winui.json`, REACTIVITY.md 4.1); `Unknown` when there is no fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObsClass {
    Unknown,
    Constant,
    SetOnly,
    Exact,
    Partial,
    None,
}

/// ABI v2.6 `observe` facts of a prop, parallel to `PROPS` (`names::OBSERVE`). The mechanisms themselves are the
/// generated `dispatch::OBSERVE_CONNECT` functions (DP callbacks and events).
pub struct Observe {
    pub class: ObsClass,
    /// `value_cost: proportional`: the backend does not report the prop while `set_prop` writes it (REACTIVITY.md 5.4).
    pub proportional: bool,
}

/// The observe facts of prop `id`.
pub fn observe(id: u32) -> Option<&'static Observe> {
    crate::names::OBSERVE.get((id as usize).checked_sub(1)?)
}

use crate::names::{ATTRS, EVENTS, KINDS, METHODS, PROPS, SLOTS};

pub fn kind(id: u32) -> Option<&'static Kind> {
    KINDS.get((id as usize).checked_sub(1)?)
}

pub fn kind_by_tag(tag: &str) -> u32 {
    KINDS.iter().position(|k| k.tag == tag).map_or(0, |i| i as u32 + 1)
}

/// The id of the member named `name` whose class is nearest in `kind`'s chain, or 0.
fn in_chain<T>(table: &[T], kind: u32, name: &str, class_of: impl Fn(&T) -> u16, name_of: impl Fn(&T) -> &str) -> u32 {
    let Some(k) = self::kind(kind) else { return 0 };
    for &c in k.chain {
        if let Some(i) = table.iter().position(|m| class_of(m) == c && name_of(m) == name) {
            return i as u32 + 1;
        }
    }
    0
}

pub fn prop_id(kind: u32, name: &str) -> u32 {
    in_chain(PROPS, kind, name, |p| p.class, |p| p.name)
}

pub fn event_id(kind: u32, name: &str) -> u32 {
    in_chain(EVENTS, kind, name, |e| e.class, |e| e.name)
}

pub fn slot_id(kind: u32, name: &str) -> u32 {
    in_chain(SLOTS, kind, name, |s| s.class, |s| s.name)
}

pub fn method_id(kind: u32, name: &str) -> u32 {
    in_chain(METHODS, kind, name, |m| m.class, |m| m.name)
}

/// Attributes are scoped by the parent's kind: `resolve(SVL_ATTR, grid, "row")`.
pub fn attr_id(parent_kind: u32, name: &str) -> u32 {
    in_chain(ATTRS, parent_kind, name, |a| a.owner, |a| a.name)
}

/// Whether member `class` applies to `kind` (its class is in the kind's chain).
pub fn applies(kind: u32, class: u16) -> bool {
    self::kind(kind).is_some_and(|k| k.chain.contains(&class))
}

pub fn prop(id: u32) -> Option<&'static Prop> {
    PROPS.get((id as usize).checked_sub(1)?)
}
pub fn event(id: u32) -> Option<&'static Event> {
    EVENTS.get((id as usize).checked_sub(1)?)
}
pub fn slot(id: u32) -> Option<&'static Slot> {
    SLOTS.get((id as usize).checked_sub(1)?)
}
pub fn attr(id: u32) -> Option<&'static Attr> {
    ATTRS.get((id as usize).checked_sub(1)?)
}
pub fn method(id: u32) -> Option<&'static Method> {
    METHODS.get((id as usize).checked_sub(1)?)
}

/// Native PascalCase to kebab-case, the naming rule for tags and members (`IsChecked` -> `is-checked`).
pub fn kebab(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            let prev = chars[i - 1];
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
            if prev.is_ascii_lowercase() || prev.is_ascii_digit() || (prev.is_ascii_uppercase() && next_lower) {
                out.push('-');
            }
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::names::*;
    use std::collections::HashSet;

    #[test]
    fn kebab_rule() {
        assert_eq!(kebab("StackPanel"), "stack-panel");
        assert_eq!(kebab("TextBlock"), "text-block");
        assert_eq!(kebab("Window"), "window");
        assert_eq!(kebab("NavigationView"), "navigation-view");
        assert_eq!(kebab("IsChecked"), "is-checked");
        assert_eq!(kebab("UIElement"), "ui-element");
    }

    #[test]
    fn tags_and_members_follow_the_naming_rule() {
        for k in KINDS {
            let short = k.native.rsplit('.').next().unwrap();
            assert_eq!(k.tag, kebab(short), "tag of {}", k.native);
        }
        for p in PROPS {
            assert_eq!(p.name, kebab(p.native));
        }
        for e in EVENTS {
            if e.notify {
                assert_eq!(e.name, format!("notify::{}", kebab(e.native)));
                assert!(e.payload.is_empty() && !e.sync && !e.close);
                let p = PROPS.iter().find(|p| p.class == e.class && p.native == e.native);
                assert!(p.is_some_and(|p| p.read && p.write), "{}: no read-write prop {} on its class", e.name, e.native);
            } else {
                assert_eq!(e.name, kebab(e.native));
            }
        }
        for m in METHODS {
            assert_eq!(m.name, kebab(m.native));
        }
        for a in ATTRS {
            assert_eq!(a.name, kebab(a.native));
        }
    }

    #[test]
    fn names_are_unique_per_kind() {
        for (ki, k) in KINDS.iter().enumerate() {
            let kid = ki as u32 + 1;
            let mut seen = HashSet::new();
            for p in PROPS.iter().filter(|p| applies(kid, p.class)) {
                // A subclass may redeclare a base prop (TextBlock.FontSize vs Control.FontSize are on
                // different chains; StackPanel.Padding has no base Padding): resolution picks the nearest.
                seen.insert(p.name);
            }
            for name in &seen {
                let id = prop_id(kid, name);
                assert!(id > 0, "<{}> resolves {name}", k.tag);
                assert!(applies(kid, PROPS[id as usize - 1].class));
            }
        }
    }

    #[test]
    fn counter_kinds_resolve() {
        let window = kind_by_tag("window");
        let stack = kind_by_tag("stack-panel");
        let text = kind_by_tag("text-block");
        let button = kind_by_tag("button");
        assert!(window > 0 && stack > 0 && text > 0 && button > 0);
        assert!(kind(window).unwrap().toplevel);
        assert_eq!(kind(text).unwrap().text_content, Some("text"));
        assert_eq!(kind(button).unwrap().text_content, Some("content"));
        assert!(prop_id(window, "title") > 0);
        assert!(prop_id(stack, "spacing") > 0 && prop_id(stack, "orientation") > 0);
        assert_eq!(prop_id(stack, "content"), 0, "stack-panel has no content prop");
        assert!(event_id(button, "click") > 0);
        assert_eq!(event_id(text, "click"), 0);
        assert!(event_id(window, "closed") > 0 && EVENTS[event_id(window, "closed") as usize - 1].close);
        assert!(method_id(window, "activate") > 0);
        // default slots: content for window and button, children for panels, none for text-block
        assert_eq!(slot(kind(window).unwrap().default_slot).unwrap().name, "content");
        assert_eq!(slot(kind(stack).unwrap().default_slot).unwrap().name, "children");
        assert_eq!(kind(text).unwrap().default_slot, 0);
        // inherited members
        assert!(prop_id(button, "margin") > 0 && prop_id(button, "is-enabled") > 0);
        assert!(event_id(stack, "key-down") > 0);
    }

    #[test]
    fn grid_attrs_are_scoped_by_parent() {
        let grid = kind_by_tag("grid");
        let stack = kind_by_tag("stack-panel");
        for n in ["row", "column", "row-span", "column-span"] {
            let id = attr_id(grid, n);
            assert!(id > 0, "grid attr {n}");
            assert_eq!(slot(attr(id).unwrap().slot).unwrap().name, "children");
            assert_eq!(attr_id(stack, n), 0, "stack-panel has no {n}");
        }
    }

    #[test]
    fn sync_events_are_the_settable_ones() {
        let tb = kind_by_tag("text-box");
        let e = event(event_id(tb, "before-text-changing")).unwrap();
        assert!(e.sync && e.payload == ["newText"]);
        assert!(!event(event_id(tb, "text-changed")).unwrap().sync);
    }

    #[test]
    fn enum_tables_are_complete() {
        for p in PROPS {
            if let Ty::Enum(i) = p.ty {
                let e = &ENUMS[i as usize];
                assert!(!e.members.is_empty(), "{}", e.native);
            }
        }
    }
}
