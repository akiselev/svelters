//! Shapes of the generated non-GUI type tables (`generated/types_tables.rs`, N4) and the lookups over them.
//!
//! The generated layer is data only (NON-GUI-TYPES.md section 2.4: "generated Rust tables are `const` data with no
//! `unsafe`"): every bound WinRT kind with its members at the ABI level (interface IID index, vtable slot, parameter
//! types). The hand-written core (`types/` on Windows) reads these rows and calls the vtable slot through the dynamic
//! invoker (`invoke.rs`), so no per-member native code is generated.
//!
//! Platform-neutral: compiled and unit-tested on every target.

/// Index into [`crate::types::tables::TYS`].
pub type TyId = u16;

/// How a value crosses: the WinRT ABI type of a slot plus what JS sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Void,
    Bool,
    Char16,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
    /// HSTRING.
    Str,
    Guid,
    /// `IInspectable` (`object`): boxed scalars (`IPropertyValue`) unbox, other objects are handles.
    Variant,
    /// `Windows.Foundation.DateTime` (100 ns ticks since 1601): a JS `Date`.
    DateTime,
    /// `Windows.Foundation.TimeSpan` (100 ns): milliseconds.
    TimeSpan,
    /// Index into `ENUMS` (flags too: `EnumT::flags`).
    Enum(u16),
    /// Index into `RECORDS`: a plain JS object by field name.
    Record(u16),
    /// A runtime class or interface. `kind`: index into `KINDS` it wraps as (0 = the IInspectable root);
    /// `iid`: index into `IIDS` an argument is queried for (the class's default interface).
    Object { kind: u16, iid: u16 },
    /// A UIElement-derived class: an element handle in; our element handle (or null) out.
    Widget { iid: u16 },
    /// `IReference<T>`: null or T. `iid`: IReference<T>.
    Nullable { of: TyId, iid: u16 },
    /// `IVectorView<T>`: a JS array (snapshot).
    View { of: TyId },
    /// `IIterable<T>`: a JS array (snapshot, through `First`/`MoveNext`).
    Iterable { of: TyId },
    /// `IMapView<String, V>`: a plain object (snapshot); `iter_iid` is `IIterable<IKeyValuePair<String, V>>`.
    MapView { of: TyId, iter_iid: u16 },
    /// `IKeyValuePair<K, V>`: `{key, value}`.
    Pair { key: TyId, of: TyId },
    /// `IAsyncOperation<T>` (`of` = T) / `IAsyncAction` (`of` = Void); `handler_iid` is the Completed handler's IID.
    /// `progress`: the `...WithProgress` variants (two more vtable slots before Completed).
    Async { of: TyId, handler_iid: u16, progress: bool },
}

/// `(iid index, vtable slot)`; `iid == 0` means absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Acc {
    pub iid: u16,
    pub slot: u16,
}

impl Acc {
    pub const NONE: Acc = Acc { iid: 0, slot: 0 };
    pub fn is_some(&self) -> bool {
        self.iid != 0
    }
}

/// Method row flags.
pub const M_STATIC: u8 = 1;
/// A constructor (`new`): activation factory method or `ActivateInstance`.
pub const M_CTOR: u8 = 2;
/// `IActivationFactory::ActivateInstance` (the default constructor).
pub const M_ACTIVATE: u8 = 4;
/// A composable factory: two trailing ABI parameters (outer = null, out inner) are hidden.
pub const M_COMPOSABLE: u8 = 8;
/// Returns `IAsync*`: a Promise (`SVL_M_ASYNC`).
pub const M_ASYNC: u8 = 16;

#[derive(Debug)]
pub struct Method {
    /// Declaring kind (index into `KINDS`).
    pub kind: u16,
    pub native: &'static str,
    /// The interface the slot belongs to (index into `IIDS`); for statics and constructors the factory interface,
    /// for `M_ACTIVATE` IActivationFactory.
    pub iid: u16,
    pub slot: u16,
    pub flags: u8,
    pub params: &'static [TyId],
    pub names: &'static [&'static str],
    /// `out` parameters (S2 convention: the call returns `[ret, outs...]`).
    pub outs: &'static [TyId],
    pub out_names: &'static [&'static str],
    pub ret: TyId,
}

/// Group flags.
pub const G_STATIC: u8 = 1;
pub const G_CTOR: u8 = 2;

/// One JS method name of a kind: its overloads (picked by argument count at run time).
pub struct Group {
    pub name: &'static str,
    pub kind: u16,
    pub flags: u8,
    /// Indices into `METHODS`, by ascending arity.
    pub methods: &'static [u16],
}

pub struct Prop {
    pub name: &'static str,
    pub native: &'static str,
    pub kind: u16,
    pub ty: TyId,
    pub get: Acc,
    pub set: Acc,
    /// The `FooProperty` static getter of a DependencyProperty-backed prop (observable).
    pub dp: Acc,
}

pub struct Event {
    /// Kebab-case (`map-changed`).
    pub name: &'static str,
    pub native: &'static str,
    pub kind: u16,
    pub iid: u16,
    /// `add_X` / `remove_X` slots.
    pub add: u16,
    pub remove: u16,
    /// IID of the delegate type (what our handler answers to QueryInterface).
    pub delegate_iid: u16,
    /// Invoke parameter types (integer-class only: pointers, ints, enums).
    pub args: &'static [TyId],
}

/// Thread class (NON-GUI-TYPES.md section 4.1).
pub const T_ANY: u8 = 0;
pub const T_UI: u8 = 1;
pub const T_DISPATCHER: u8 = 2;

pub struct Kind {
    /// Full native name (`Windows.Foundation.Uri`, or a generic instance `Windows.Foundation.Collections.IVector<string>`).
    pub name: &'static str,
    /// Index + 1 of the nearest bound base kind; 0 at the root.
    pub parent: u16,
    /// A runtime class (activatable by name, statics through its activation factory).
    pub class: bool,
    pub thread: u8,
    /// `DispatcherQueue` getter for `T_DISPATCHER`.
    pub thread_getter: Acc,
    /// Indices into `GROUPS`, sorted by (name, flags).
    pub groups: &'static [u16],
    /// Indices into `PROPS`, sorted by name.
    pub props: &'static [u16],
    /// Indices into `EVENTS`, sorted by name.
    pub events: &'static [u16],
}

pub struct EnumT {
    pub name: &'static str,
    pub flags: bool,
    pub unsigned: bool,
    /// `(native name, value)`.
    pub members: &'static [(&'static str, i64)],
}

pub struct Field {
    /// camelCase JS name.
    pub name: &'static str,
    pub ty: TyId,
    pub offset: u16,
}

pub struct RecordT {
    pub name: &'static str,
    pub size: u16,
    pub align: u16,
    pub fields: &'static [Field],
}

/// Interfaces the core calls itself, resolved from the metadata by the generator (never typed by hand).
pub struct Core {
    /// `IPropertyValue`: `Type` getter and the scalar getters, by `PropertyType` value (0 when not supported).
    pub property_value: u16,
    pub pv_type: u16,
    pub pv_get: &'static [(u16, u16)],
    /// `IPropertyValueStatics` (activation factory of `Windows.Foundation.PropertyValue`): `Create*` slots by
    /// `PropertyType` value.
    pub pv_statics: u16,
    pub pv_create: &'static [(u16, u16)],
    /// `IStringable.ToString`.
    pub stringable: Acc,
    /// `IDispatcherQueue.HasThreadAccess`.
    pub dq_has_thread_access: Acc,
    /// `IDependencyObject` (QueryInterface target for DP observation).
    pub dependency_object: u16,
    /// Collections: `IVectorView<T>.GetAt` / `Size`, `IIterable<T>.First`, `IIterator<T>.Current` / `HasCurrent` /
    /// `MoveNext`, `IKeyValuePair<K, V>.Key` / `Value`, `IReference<T>.Value` (slots are the same for every T).
    pub view_get_at: u16,
    pub view_size: u16,
    pub iterable_first: u16,
    pub iterator_current: u16,
    pub iterator_has_current: u16,
    pub iterator_move_next: u16,
    pub pair_key: u16,
    pub pair_value: u16,
    pub reference_value: u16,
    /// `IAsyncInfo` and its `Status`, `ErrorCode`, `Cancel`, `Close` slots.
    pub async_info: u16,
    pub info_status: u16,
    pub info_error_code: u16,
    pub info_cancel: u16,
    pub info_close: u16,
    /// `put_Completed` / `GetResults` slots: without progress (`IAsyncAction`, `IAsyncOperation<T>`) and with.
    pub put_completed: u16,
    pub get_results: u16,
    pub put_completed_progress: u16,
    pub get_results_progress: u16,
    /// `AsyncStatus` values.
    pub status_completed: i32,
    pub status_canceled: i32,
    pub status_error: i32,
}

/// Id spaces of the type layer (element ids are small, `index + 1`).
pub const KIND_BASE: u32 = 0x20000;
pub const GROUP_BASE: u32 = 0x20000;
pub const PROP_BASE: u32 = 0x20000;
pub const EVENT_BASE: u32 = 0x20000;

use super::tables::{ENUMS, EVENTS, GROUPS, KINDS, METHODS, PROPS};

pub fn is_type_kind(kind: u32) -> bool {
    kind >= KIND_BASE && ((kind - KIND_BASE) as usize) < KINDS.len()
}

pub fn kind(kind: u32) -> Option<&'static Kind> {
    if kind < KIND_BASE {
        return None;
    }
    KINDS.get((kind - KIND_BASE) as usize)
}

/// Kind id of a native name (`Windows.Foundation.Uri`); 0 when unbound.
pub fn kind_by_name(name: &str) -> u32 {
    KINDS.iter().position(|k| k.name == name).map_or(0, |i| KIND_BASE + i as u32)
}

/// The kind and its bases, nearest first (indices into `KINDS`).
pub fn chain(kind: u32) -> Vec<u16> {
    let mut out = Vec::new();
    let mut cur = kind.checked_sub(KIND_BASE).map(|k| k as u16);
    while let Some(k) = cur {
        if out.contains(&k) || (k as usize) >= KINDS.len() {
            break;
        }
        out.push(k);
        let p = KINDS[k as usize].parent;
        cur = (p != 0).then(|| p - 1);
    }
    out
}

/// `resolve(SVL_METHOD, kind, name)`: an instance method group first, then a static one; `static:<name>` only
/// statics; `new` the constructors. Walks the base chain (a derived kind's group shadows its base's).
pub fn group_id(kind: u32, name: &str) -> u32 {
    let (want_static, name) = match name.strip_prefix("static:") {
        Some(n) => (true, n),
        None => (false, name),
    };
    for k in chain(kind) {
        let groups = KINDS[k as usize].groups;
        let found = groups.iter().copied().filter(|g| GROUPS[*g as usize].name == name);
        let mut best: Option<u16> = None;
        for g in found {
            let st = GROUPS[g as usize].flags & G_STATIC != 0;
            if want_static && !st {
                continue;
            }
            if best.is_none() || !st {
                best = Some(g);
            }
        }
        if let Some(g) = best {
            return GROUP_BASE + g as u32;
        }
    }
    0
}

pub fn group(id: u32) -> Option<&'static Group> {
    GROUPS.get(id.checked_sub(GROUP_BASE)? as usize)
}

/// Whether `kind` is the group's kind or derives from it.
pub fn applies(kind: u32, owner: u16) -> bool {
    chain(kind).contains(&owner)
}

pub fn prop_id(kind: u32, name: &str) -> u32 {
    for k in chain(kind) {
        if let Some(p) = KINDS[k as usize].props.iter().find(|p| PROPS[**p as usize].name == name) {
            return PROP_BASE + *p as u32;
        }
    }
    0
}

pub fn prop(id: u32) -> Option<&'static Prop> {
    PROPS.get(id.checked_sub(PROP_BASE)? as usize)
}

pub fn event_id(kind: u32, name: &str) -> u32 {
    for k in chain(kind) {
        if let Some(e) = KINDS[k as usize].events.iter().find(|e| EVENTS[**e as usize].name == name) {
            return EVENT_BASE + *e as u32;
        }
    }
    0
}

pub fn event(id: u32) -> Option<&'static Event> {
    EVENTS.get(id.checked_sub(EVENT_BASE)? as usize)
}

/// The overload of a group for `n` JS arguments: the one with exactly `n` parameters, else an error naming the
/// arities there are.
pub fn pick(g: &Group, n: usize) -> Result<&'static Method, String> {
    for m in g.methods {
        let m = &METHODS[*m as usize];
        if m.params.len() == n {
            return Ok(m);
        }
    }
    let ar: Vec<String> = g.methods.iter().map(|m| METHODS[*m as usize].params.len().to_string()).collect();
    Err(format!("{}: no overload takes {n} argument(s) (takes {})", g.name, ar.join(" or ")))
}

/// Smallest and largest arity of a group.
pub fn arity_range(g: &Group) -> (usize, usize) {
    let ns = g.methods.iter().map(|m| METHODS[*m as usize].params.len());
    (ns.clone().min().unwrap_or(0), ns.max().unwrap_or(0))
}

pub fn enum_t(i: u16) -> &'static EnumT {
    &ENUMS[i as usize]
}

/// `MapChanged` -> `map-changed`; `UIElement` -> `ui-element`.
pub fn kebab(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    for (i, c) in cs.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let prev_lower = i > 0 && (cs[i - 1].is_ascii_lowercase() || cs[i - 1].is_ascii_digit());
            let next_lower = cs.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
            if i > 0 && (prev_lower || (next_lower && cs[i - 1].is_ascii_uppercase())) {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(*c);
        }
    }
    out
}

/// Enum member names compare without case and dashes: `ReplaceExisting`, `replaceExisting`, `replace-existing`.
pub fn norm_name(s: &str) -> String {
    s.chars().filter(|c| *c != '-' && *c != '_').flat_map(|c| c.to_lowercase()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::tables::*;

    #[test]
    fn kebab_names() {
        assert_eq!(kebab("MapChanged"), "map-changed");
        assert_eq!(kebab("UIElement"), "ui-element");
        assert_eq!(kebab("Tick"), "tick");
        assert_eq!(norm_name("Replace-Existing"), "replaceexisting");
    }

    #[test]
    fn every_kind_resolves_by_name() {
        for (i, k) in KINDS.iter().enumerate() {
            assert_eq!(kind_by_name(k.name), KIND_BASE + i as u32, "{}", k.name);
        }
    }

    #[test]
    fn tables_are_consistent() {
        for (i, k) in KINDS.iter().enumerate() {
            for g in k.groups {
                assert_eq!(GROUPS[*g as usize].kind as usize, i, "{}: group {}", k.name, GROUPS[*g as usize].name);
            }
            for p in k.props {
                assert_eq!(PROPS[*p as usize].kind as usize, i);
            }
            for e in k.events {
                assert_eq!(EVENTS[*e as usize].kind as usize, i);
            }
            if k.parent != 0 {
                assert!((k.parent as usize) <= KINDS.len());
            }
        }
        for g in GROUPS {
            let mut last = None;
            for m in g.methods {
                let n = METHODS[*m as usize].params.len();
                assert!(last.is_none_or(|l| l < n), "{}: arities must be distinct and ascending", g.name);
                last = Some(n);
            }
        }
        for t in TYS {
            match *t {
                Ty::Enum(e) => assert!((e as usize) < ENUMS.len()),
                Ty::Record(r) => assert!((r as usize) < RECORDS.len()),
                Ty::Object { kind, iid } => assert!((kind as usize) < KINDS.len() && (iid as usize) < IIDS.len()),
                Ty::Nullable { of, .. } | Ty::View { of } | Ty::Iterable { of } | Ty::MapView { of, .. } | Ty::Async { of, .. } => {
                    assert!((of as usize) < TYS.len())
                }
                _ => {}
            }
        }
        for r in RECORDS {
            for f in r.fields {
                assert!(f.offset < r.size, "{}.{}", r.name, f.name);
            }
            assert_eq!(r.size % r.align, 0, "{}", r.name);
        }
    }

    #[test]
    fn uri_members_resolve() {
        let k = kind_by_name("Windows.Foundation.Uri");
        assert_ne!(k, 0);
        let g = group(group_id(k, "new")).expect("constructors");
        assert_eq!(arity_range(g), (1, 2));
        assert!(pick(g, 3).unwrap_err().contains("takes 1 or 2"));
        assert_ne!(prop_id(k, "host"), 0);
        assert_eq!(prop_id(k, "nope"), 0);
        let esc = group(group_id(k, "escapeComponent")).unwrap();
        assert!(esc.flags & G_STATIC != 0);
    }

    #[test]
    fn inherited_members_resolve_through_the_chain() {
        let k = kind_by_name("Microsoft.UI.Xaml.Media.SolidColorBrush");
        assert_ne!(prop_id(k, "color"), 0);
        // Brush.Opacity is declared on the base.
        let p = prop(prop_id(k, "opacity")).unwrap();
        assert_eq!(KINDS[p.kind as usize].name, "Microsoft.UI.Xaml.Media.Brush");
        assert!(applies(k, p.kind));
        assert!(prop(prop_id(k, "color")).unwrap().dp.is_some());
    }
}
