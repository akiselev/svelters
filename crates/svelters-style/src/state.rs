//! Node state bits and the state pseudo-classes that test them.
//!
//! The layout is the scene's `StateBits` (ARCH-EMBED §8.4, `svelters-egui/src/provider/state.rs`):
//! bits 0–31 are svelters' built-in states, with the same bit for the same state, and bits 32–63
//! are provider states, allocated by name at startup. This crate links no toolkit, so it keeps its
//! own copy of the built-in bits; CS5e's `Element` impl for the scene passes `StateBits.0` through
//! unchanged and its tests pin the two tables together.

/// A set of node states: bit `i` set = state `i` on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StateMask(pub u64);

impl StateMask {
    pub const NONE: StateMask = StateMask(0);
    pub const HOVERED: StateMask = StateMask(1 << 0);
    pub const PRESSED: StateMask = StateMask(1 << 1);
    pub const FOCUSED: StateMask = StateMask(1 << 2);
    pub const FOCUS_WITHIN: StateMask = StateMask(1 << 3);
    pub const DISABLED: StateMask = StateMask(1 << 4);
    pub const CHECKED: StateMask = StateMask(1 << 5);
    pub const SELECTED: StateMask = StateMask(1 << 6);
    pub const OPEN: StateMask = StateMask(1 << 7);
    pub const EXPANDED: StateMask = StateMask(1 << 8);
    pub const INVALID: StateMask = StateMask(1 << 9);
    pub const READ_ONLY: StateMask = StateMask(1 << 10);
    pub const DRAGGING: StateMask = StateMask(1 << 11);
    pub const EXITING: StateMask = StateMask(1 << 12);
    pub const BUSY: StateMask = StateMask(1 << 13);
    // ARCH-WIDGETS §2.2 (OW-CSS): bits 14–22.
    pub const FOCUS_VISIBLE: StateMask = StateMask(1 << 14);
    pub const INDETERMINATE: StateMask = StateMask(1 << 15);
    pub const PLACEHOLDER_SHOWN: StateMask = StateMask(1 << 16);
    pub const DEFAULT: StateMask = StateMask(1 << 17);
    pub const DROP_TARGET: StateMask = StateMask(1 << 18);
    pub const CURRENT: StateMask = StateMask(1 << 19);
    pub const BACKDROP: StateMask = StateMask(1 << 20);
    pub const REQUIRED: StateMask = StateMask(1 << 21);
    pub const USER_INVALID: StateMask = StateMask(1 << 22);
    // WIDGET-PARITY P5 (OW-CSS): tree rows with children; 24–31 stay free.
    pub const HAS_CHILDREN: StateMask = StateMask(1 << 23);

    /// The built-in states by the scene's names (`StateBits::BUILTIN`).
    pub const BUILTIN: &'static [(&'static str, StateMask)] = &[
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

    pub const fn contains(self, other: StateMask) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn intersects(self, other: StateMask) -> bool {
        self.0 & other.0 != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn set(&mut self, bits: StateMask, on: bool) {
        if on {
            self.0 |= bits.0;
        } else {
            self.0 &= !bits.0;
        }
    }
}

impl std::ops::BitOr for StateMask {
    type Output = StateMask;
    fn bitor(self, rhs: StateMask) -> StateMask {
        StateMask(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for StateMask {
    fn bitor_assign(&mut self, rhs: StateMask) {
        self.0 |= rhs.0;
    }
}

/// The CSS pseudo-classes that test one built-in bit: `(name, bit, negated)`. `:enabled` is
/// `:not(:disabled)`, `:valid` is `:not(:invalid)`, `:read-write` is `:not(:read-only)`,
/// `:optional` is `:not(:required)`. `:focus-visible` is not here: it is
/// [`Element::is_focus_visible`](crate::Element::is_focus_visible), which reads
/// [`StateMask::FOCUS_VISIBLE`].
pub const STATE_PSEUDO_CLASSES: &[(&str, StateMask, bool)] = &[
    ("hover", StateMask::HOVERED, false),
    ("active", StateMask::PRESSED, false),
    ("focus", StateMask::FOCUSED, false),
    ("focus-within", StateMask::FOCUS_WITHIN, false),
    ("disabled", StateMask::DISABLED, false),
    ("enabled", StateMask::DISABLED, true),
    ("checked", StateMask::CHECKED, false),
    ("selected", StateMask::SELECTED, false),
    ("open", StateMask::OPEN, false),
    ("expanded", StateMask::EXPANDED, false),
    ("invalid", StateMask::INVALID, false),
    ("valid", StateMask::INVALID, true),
    ("read-only", StateMask::READ_ONLY, false),
    ("read-write", StateMask::READ_ONLY, true),
    ("dragging", StateMask::DRAGGING, false),
    ("exiting", StateMask::EXITING, false),
    ("busy", StateMask::BUSY, false),
    // ARCH-WIDGETS §2.2 (OW-CSS)
    ("indeterminate", StateMask::INDETERMINATE, false),
    ("placeholder-shown", StateMask::PLACEHOLDER_SHOWN, false),
    ("default", StateMask::DEFAULT, false),
    ("drop-target", StateMask::DROP_TARGET, false),
    ("current", StateMask::CURRENT, false),
    ("backdrop", StateMask::BACKDROP, false),
    ("required", StateMask::REQUIRED, false),
    ("optional", StateMask::REQUIRED, true),
    ("user-invalid", StateMask::USER_INVALID, false),
    // WIDGET-PARITY P5
    ("has-children", StateMask::HAS_CHILDREN, false),
];

/// Resolves provider state names (`:state(cad.pending)`) to their bit at parse time. The scene's
/// registry (`svelters_egui::state_registry`) implements it in CS5e.
pub trait StateNames {
    /// The bit of the provider state `name` as written inside `:state()`, e.g. `cad.pending`.
    fn lookup(&self, name: &str) -> Option<StateMask>;
}

/// A fixed name → bit list, for tests and hosts with a static set.
impl<const N: usize> StateNames for [(&str, StateMask); N] {
    fn lookup(&self, name: &str) -> Option<StateMask> {
        self.iter().find(|(n, _)| *n == name).map(|(_, m)| *m)
    }
}
