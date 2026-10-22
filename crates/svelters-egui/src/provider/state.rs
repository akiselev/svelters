//! Node state bits (ARCH-EMBED §8.4): [`StateBits`], svelters' built-in bits 0–31 and the registry
//! that hands bits 32–63 to provider names ([`StateBit`], [`state_bits!`](crate::state_bits)).
//!
//! The walk will set the built-in bits from widget responses, and the style hook (ST3), a11y and the
//! MCP introspection read them; RA1 lands the storage, the registry and `WidgetCx::set_state`.
//! SG1 moves the bits into `NodeCore` (they stay a field there, never an extension column).
//!
//! SG2 hooks the bits into the scene's journal: a change is a `Change::State { before, after }`
//! record, and observers filter by bit (`ChangeFilter::states`, built-in or provider bits), so a
//! provider toggling its own bits on 5k nodes costs nothing for observers of other bits.
//!
//! Writer lane: RUSTAPI (RA1); SCENE (SG2) for the journal hookup.

use crate::scene::Node;
use std::sync::OnceLock;

/// The state of one node: bit `i` set = state `i` on. Bits 0–31 are svelters' built-ins, 32–63 are
/// provider states allocated at startup.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StateBits(pub u64);

impl StateBits {
    pub const NONE: StateBits = StateBits(0);
    // The built-in states (scene-node §1 `StateFlags`).
    pub const HOVERED: StateBits = StateBits(1 << 0);
    pub const PRESSED: StateBits = StateBits(1 << 1);
    pub const FOCUSED: StateBits = StateBits(1 << 2);
    pub const FOCUS_WITHIN: StateBits = StateBits(1 << 3);
    pub const DISABLED: StateBits = StateBits(1 << 4);
    pub const CHECKED: StateBits = StateBits(1 << 5);
    pub const SELECTED: StateBits = StateBits(1 << 6);
    pub const OPEN: StateBits = StateBits(1 << 7);
    pub const EXPANDED: StateBits = StateBits(1 << 8);
    pub const INVALID: StateBits = StateBits(1 << 9);
    pub const READ_ONLY: StateBits = StateBits(1 << 10);
    pub const DRAGGING: StateBits = StateBits(1 << 11);
    pub const EXITING: StateBits = StateBits(1 << 12);
    pub const BUSY: StateBits = StateBits(1 << 13);
    // OW0 (ARCH-WIDGETS §2.2): the own widgets' states. OW-CSS adds the same rows, with the same
    // bits, to svelters-style's table and its pseudo-classes.
    /// `:focus-visible`: focused by keyboard or program while the last input was a key (Focusable).
    pub const FOCUS_VISIBLE: StateBits = StateBits(1 << 14);
    /// `:indeterminate`: a tristate check box in its third state, a progress bar without a value.
    pub const INDETERMINATE: StateBits = StateBits(1 << 15);
    /// `:placeholder-shown`: an empty text field showing its placeholder (TextInput).
    pub const PLACEHOLDER_SHOWN: StateBits = StateBits(1 << 16);
    /// `:default`: the scope's default button (the `default` prop).
    pub const DEFAULT: StateBits = StateBits(1 << 17);
    /// `:drop-target`: a drag it accepts is over it (DropTarget; GTK's `:drop(active)`).
    pub const DROP_TARGET: StateBits = StateBits(1 << 18);
    /// `:current`: the keyboard cursor item of a list or table (Selectable), distinct from selected.
    pub const CURRENT: StateBits = StateBits(1 << 19);
    /// `:backdrop`: its viewport does not have the OS focus (GTK's name).
    pub const BACKDROP: StateBits = StateBits(1 << 20);
    /// `:required`: the `required` prop (FM1 QW8).
    pub const REQUIRED: StateBits = StateBits(1 << 21);
    /// `:user-invalid`: invalid, shown only after the user's first commit.
    pub const USER_INVALID: StateBits = StateBits(1 << 22);
    // WIDGET-PARITY P5 (OW-CSS): tree rows with children; 24–31 stay free.
    pub const HAS_CHILDREN: StateBits = StateBits(1 << 23);

    /// The built-in names, by bit.
    pub const BUILTIN: &'static [(&'static str, StateBits)] = &[
        ("hovered", Self::HOVERED),
        ("pressed", Self::PRESSED),
        ("focused", Self::FOCUSED),
        ("focus-within", Self::FOCUS_WITHIN),
        ("disabled", Self::DISABLED),
        ("checked", Self::CHECKED),
        ("selected", Self::SELECTED),
        ("open", Self::OPEN),
        ("expanded", Self::EXPANDED),
        ("invalid", Self::INVALID),
        ("read-only", Self::READ_ONLY),
        ("dragging", Self::DRAGGING),
        ("exiting", Self::EXITING),
        ("busy", Self::BUSY),
        ("focus-visible", Self::FOCUS_VISIBLE),
        ("indeterminate", Self::INDETERMINATE),
        ("placeholder-shown", Self::PLACEHOLDER_SHOWN),
        ("default", Self::DEFAULT),
        ("drop-target", Self::DROP_TARGET),
        ("current", Self::CURRENT),
        ("backdrop", Self::BACKDROP),
        ("required", Self::REQUIRED),
        ("user-invalid", Self::USER_INVALID),
        ("has-children", Self::HAS_CHILDREN),
    ];

    pub const fn contains(self, other: StateBits) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn intersects(self, other: StateBits) -> bool {
        self.0 & other.0 != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn set(&mut self, bits: StateBits, on: bool) {
        if on {
            self.0 |= bits.0;
        } else {
            self.0 &= !bits.0;
        }
    }

    /// The names of the set bits: built-ins by their name, provider bits as `<namespace>.<name>`.
    pub fn names(self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Self::BUILTIN.iter().filter(|(_, b)| self.contains(*b)).map(|(n, _)| *n).collect();
        for (i, def) in registry().iter().enumerate() {
            if self.0 & (1u64 << (32 + i)) != 0 {
                out.push(def.full);
            }
        }
        out
    }
}

impl std::ops::BitOr for StateBits {
    type Output = StateBits;
    fn bitor(self, rhs: StateBits) -> StateBits {
        StateBits(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for StateBits {
    fn bitor_assign(&mut self, rhs: StateBits) {
        self.0 |= rhs.0;
    }
}

/// A provider state, declared with [`state_bits!`](crate::state_bits). Its bit (32–63) comes from
/// the registry built at startup from every declared state, sorted by full name, so a binary always
/// gets the same bits.
pub struct StateBit {
    pub namespace: &'static str,
    pub name: &'static str,
    /// `<namespace>.<name>` (written by the macro).
    pub full: &'static str,
}

impl StateBit {
    #[doc(hidden)]
    pub const fn new(namespace: &'static str, name: &'static str, full: &'static str) -> StateBit {
        StateBit { namespace, name, full }
    }

    /// The bit, as a [`StateBits`] mask.
    pub fn bits(&self) -> StateBits {
        let i = registry().iter().position(|d| d.full == self.full).expect("a state declared with state_bits! is registered");
        StateBits(1u64 << (32 + i))
    }
}

/// What `state_bits!` registers (one per state).
#[doc(hidden)]
pub struct StateBitDef(pub &'static StateBit);

inventory::collect!(StateBitDef);

/// The provider states of this process, by bit (bit `32 + index`). Duplicate names and more than 32
/// states fail at startup (the first call).
pub fn registry() -> &'static [&'static StateBit] {
    static R: OnceLock<Vec<&'static StateBit>> = OnceLock::new();
    R.get_or_init(|| {
        let mut v: Vec<&'static StateBit> = inventory::iter::<StateBitDef>.into_iter().map(|d| d.0).collect();
        v.sort_by_key(|d| d.full);
        for w in v.windows(2) {
            assert!(w[0].full != w[1].full, "svelters-egui: provider state `{}` is declared twice", w[0].full);
        }
        assert!(v.len() <= 32, "svelters-egui: {} provider states declared; bits 32-63 hold 32", v.len());
        v
    })
}

/// A provider state by its full name (`cad.invalid`) or a built-in name (`invalid`).
pub fn by_name(name: &str) -> Option<StateBits> {
    if let Some((_, b)) = StateBits::BUILTIN.iter().find(|(n, _)| *n == name) {
        return Some(*b);
    }
    registry().iter().position(|d| d.full == name).map(|i| StateBits(1u64 << (32 + i)))
}

/// Anything that names state bits: a [`StateBits`] mask or a provider [`StateBit`].
pub trait IntoStateBits {
    fn state_bits(&self) -> StateBits;
}

impl IntoStateBits for StateBits {
    fn state_bits(&self) -> StateBits {
        *self
    }
}

impl IntoStateBits for StateBit {
    fn state_bits(&self) -> StateBits {
        self.bits()
    }
}

impl<T: IntoStateBits + ?Sized> IntoStateBits for &T {
    fn state_bits(&self) -> StateBits {
        (**self).state_bits()
    }
}

/// The pre-SG1 node view's reader (SG1: `NodeRef::state`, `NodeMut::set_state`; the bits live in
/// `NodeCore`).
impl Node<'_> {
    pub fn state(&self) -> StateBits {
        self.state
    }
}

/// Declares provider states, each registered at startup and given a bit in 32–63:
///
/// ```ignore
/// svelters_egui::state_bits! { pub mod cad { PENDING = "pending", INVALID = "invalid" } }
/// cx.set_state(&cad::INVALID, true);
/// ```
#[macro_export]
macro_rules! state_bits {
    ($vis:vis mod $ns:ident { $($name:ident = $text:literal),* $(,)? }) => {
        $vis mod $ns {
            $(
                pub static $name: $crate::provider::StateBit =
                    $crate::provider::StateBit::new(stringify!($ns), $text, concat!(stringify!($ns), ".", $text));
                $crate::provider::__private::inventory::submit! { $crate::provider::StateBitDef(&$name) }
            )*
        }
    };
}
