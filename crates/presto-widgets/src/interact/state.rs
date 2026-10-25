//! Node state bits (`:hover`, `:active`, `:checked`, …): what units own and the UA tables and CSS
//! pseudo-classes match. Ported from svelters-egui `provider/state.rs` with the same bit numbers, so
//! svelters-style's pseudo-class table keeps matching. Provider states (bits 32–63, allocated at
//! startup) are presto-core's; this crate only names the built-ins.

/// The state of one node: bit `i` set = state `i` on. Bits 0–31 are the built-ins, 32–63 provider states.
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
    /// `:has-children`: a tree row with children (WIDGET-PARITY P5, OW-CSS); 24–31 stay free.
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

    /// The names of the set built-in bits.
    pub fn names(self) -> Vec<&'static str> {
        Self::BUILTIN.iter().filter(|(_, b)| self.contains(*b)).map(|(n, _)| *n).collect()
    }

    /// A built-in state by name.
    pub fn by_name(name: &str) -> Option<StateBits> {
        Self::BUILTIN.iter().find(|(n, _)| *n == name).map(|(_, b)| *b)
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
