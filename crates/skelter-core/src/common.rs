//! The members every node has, whatever its kind: the common props (focus order, disabled, hidden,
//! the accessible label) and the DOM-style events the document dispatches (pointer, click, wheel,
//! keys, focus). They are written as the pseudo-kind `"*"`, so their ids are
//! `PropId::of("*", name, type)` like any kind's.

use crate::ids::{PropId, SignalId};
use crate::kind::{ArgSpec, KindSpec, PropSpec, SignalFlags, SignalSpec};
use crate::value::{PropDefault, ValueType};
use skelter_base::change::ChangeClass;

const B: SignalFlags = SignalFlags::BUBBLES;
const C: SignalFlags = SignalFlags::CANCELABLE;
const P: SignalFlags = SignalFlags::PASSIVE;
const S: SignalFlags = SignalFlags::SYNC;

const fn f(a: SignalFlags, b: SignalFlags) -> SignalFlags {
    a.union(b)
}

/// The focus scope a node opens (EV3): `contain` keeps Tab inside it, `dialog` also restores focus
/// when it closes and takes the first focusable descendant when it opens.
pub const FOCUS_SCOPES: &[&str] = &["none", "contain", "dialog"];

/// A node's place in a `dialog-buttons` row (QDialogButtonBox roles); `auto` is `accept` for a
/// `default` button, `reject` for a `cancel` one, else `action`.
pub const DIALOG_ROLES: &[&str] = &["auto", "accept", "reject", "destructive", "apply", "reset", "help", "action"];

/// A node's place in a `status-bar` (QStatusBar): `temporary` (hidden while a message or status tip
/// shows), `start` and `end` (permanent).
pub const STATUS_SECTIONS: &[&str] = &["temporary", "start", "end"];

/// `aria-live`: how assistive technology announces changes inside the node.
pub const ARIA_LIVE_VALUES: &[&str] = &["off", "polite", "assertive"];

pub static COMMON: KindSpec = KindSpec {
    props: &[
        PropSpec::new("tabindex", ValueType::I64, ChangeClass::Paint),
        PropSpec::new("disabled", ValueType::Bool, ChangeClass::Paint).default(PropDefault::Bool(false)),
        PropSpec::new("hidden", ValueType::Bool, ChangeClass::Layout).default(PropDefault::Bool(false)),
        PropSpec::new("aria-label", ValueType::Str, ChangeClass::Paint),
        PropSpec::new("aria-live", ValueType::Enum, ChangeClass::Paint)
            .default(PropDefault::Enum(0))
            .enum_values(ARIA_LIVE_VALUES)
            .doc("A live region: AccessKit `live` (`off`, `polite`, `assertive`), applied after the kind's own accessibility node."),
        PropSpec::new("aria-busy", ValueType::Bool, ChangeClass::Paint)
            .default(PropDefault::Bool(false))
            .doc("The node is being updated: AccessKit `busy`, applied after the kind's own accessibility node."),
        PropSpec::new("title", ValueType::Str, ChangeClass::Paint),
        PropSpec::new("autofocus", ValueType::Bool, ChangeClass::Paint).default(PropDefault::Bool(false)),
        PropSpec::new("focus-scope", ValueType::Enum, ChangeClass::Paint).default(PropDefault::Enum(0)).enum_values(FOCUS_SCOPES),
        PropSpec::new("anchor", ValueType::Str, ChangeClass::Paint)
            .default(PropDefault::Str(""))
            .doc("A name unique in its window: the AccessKit author id, what `label for` and `Document::find_anchor` find."),
        PropSpec::new("anchor-scope", ValueType::Str, ChangeClass::Paint)
            .default(PropDefault::Str(""))
            .doc("Set by the runtime on the top nodes of a keyed `{#each}` item: the item's key and the keys around it (outermost first, separated by U+001F); anchors inside it are qualified by them. Not for apps to set."),
        PropSpec::new("invalid", ValueType::Bool, ChangeClass::Paint)
            .default(PropDefault::Bool(false))
            .doc("Marks the value as invalid: `:invalid` and AccessKit `invalid`. The app sets it; the widget never does."),
        PropSpec::new("dialog-role", ValueType::Enum, ChangeClass::Layout)
            .default(PropDefault::Enum(0))
            .enum_values(DIALOG_ROLES)
            .doc("Its place in a `dialog-buttons` row; also the `[dialog-role=…]` selector."),
        PropSpec::new("status-tip", ValueType::Str, ChangeClass::Paint)
            .default(PropDefault::Str(""))
            .doc("Status bar text while the pointer is over the node (the innermost with one wins) or, with nothing hovered, while it has focus (`Document::status_tip`)."),
        PropSpec::new("status-section", ValueType::Enum, ChangeClass::Layout)
            .default(PropDefault::Enum(0))
            .enum_values(STATUS_SECTIONS)
            .doc("Its place in a `status-bar`; also the `[status-section=…]` selector."),
        PropSpec::new("drag-data", ValueType::Str, ChangeClass::Paint).default(PropDefault::Str("")).doc("The drag payload's data (JSON for structured data); `drop` delivers it."),
        PropSpec::new("drag-type", ValueType::Str, ChangeClass::Paint)
            .default(PropDefault::Str(""))
            .doc("Makes the node a drag source: the payload's type, a MIME-like string (`cad/part`); empty for none."),
        PropSpec::new("drop-types", ValueType::Str, ChangeClass::Paint)
            .default(PropDefault::Str(""))
            .doc("Makes the node a drop target: the drag types it accepts, separated by spaces or commas (`*` any, `cad/*` a prefix); empty for none."),
    ],
    signals: &[
        SignalSpec::new("pointerdown", Some(ValueType::Record), f(f(B, C), S)),
        SignalSpec::new("pointerup", Some(ValueType::Record), f(f(B, C), S)),
        SignalSpec::new("pointermove", Some(ValueType::Record), f(B, P)),
        SignalSpec::new("pointerenter", Some(ValueType::Record), P),
        SignalSpec::new("pointerleave", Some(ValueType::Record), P),
        SignalSpec::new("pointercancel", Some(ValueType::Record), f(B, P)),
        SignalSpec::new("click", Some(ValueType::Record), f(f(B, C), S)),
        SignalSpec::new("dblclick", Some(ValueType::Record), f(f(B, C), S)),
        SignalSpec::new("contextmenu", Some(ValueType::Record), f(f(B, C), S)),
        SignalSpec::new("wheel", Some(ValueType::Record), f(f(B, C), P)),
        SignalSpec::new("keydown", Some(ValueType::Record), f(f(B, C), S)),
        SignalSpec::new("keyup", Some(ValueType::Record), f(f(B, C), S)),
        SignalSpec::new("focus", None, S),
        SignalSpec::new("blur", None, S),
        SignalSpec::new("focusin", None, f(B, S)),
        SignalSpec::new("focusout", None, f(B, S)),
        SignalSpec::new("scroll", Some(ValueType::Record), P),
        SignalSpec::new("transitionend", Some(ValueType::Record), B),
        SignalSpec::new("animationend", Some(ValueType::Record), B),
        SignalSpec::new("drag-begin", None, S).doc("Drag source: the user started dragging this node (`drag-type` set)."),
        SignalSpec::new("drag-end", Some(ValueType::Record), S)
            .args(&[ArgSpec::new("dropped", ValueType::Bool), ArgSpec::new("action", ValueType::Str)])
            .doc("Drag source: the drag ended; `dropped` when a target took it, `action` how (`move`, `copy`, `link`; `none` when not dropped)."),
        SignalSpec::new("drag-enter", Some(ValueType::Str), S)
            .args(&[ArgSpec::new("type", ValueType::Str)])
            .doc("Drop target: a drag it accepts entered it (`drop-types` set); carries the drag's type."),
        SignalSpec::new("drag-leave", None, S).doc("Drop target: the drag left, was cancelled, or was dropped here (before `drop`)."),
        SignalSpec::new("drop", Some(ValueType::Record), S)
            .args(&[
                ArgSpec::new("type", ValueType::Str),
                ArgSpec::new("data", ValueType::Str),
                ArgSpec::new("x", ValueType::F64),
                ArgSpec::new("y", ValueType::F64),
                ArgSpec::new("action", ValueType::Str),
            ])
            .doc("Drop target: a drag it accepts was released over it: type, data, the point in the node's own points, the action from the modifiers (`move`; `copy` with Ctrl, Option on macOS; `link` with Ctrl+Shift, Cmd+Option on macOS)."),
    ],
    ..KindSpec::new("*")
};

macro_rules! ids {
    ($ty:ident: $($name:ident = $s:literal : $t:literal),* $(,)?) => {
        $(pub const $name: $ty = $ty::of("*", $s, $t);)*
    };
}

ids!(PropId:
    TABINDEX = "tabindex": "int",
    DISABLED = "disabled": "bool",
    HIDDEN = "hidden": "bool",
    ARIA_LABEL = "aria-label": "string",
    ARIA_LIVE = "aria-live": "enum",
    ARIA_BUSY = "aria-busy": "bool",
    TITLE = "title": "string",
    AUTOFOCUS = "autofocus": "bool",
    FOCUS_SCOPE = "focus-scope": "enum",
    ANCHOR = "anchor": "string",
    ANCHOR_SCOPE = "anchor-scope": "string",
    INVALID = "invalid": "bool",
    DIALOG_ROLE = "dialog-role": "enum",
    STATUS_TIP = "status-tip": "string",
    STATUS_SECTION = "status-section": "enum",
    DRAG_DATA = "drag-data": "string",
    DRAG_TYPE = "drag-type": "string",
    DROP_TYPES = "drop-types": "string",
);

ids!(SignalId:
    POINTERDOWN = "pointerdown": "record",
    POINTERUP = "pointerup": "record",
    POINTERMOVE = "pointermove": "record",
    POINTERENTER = "pointerenter": "record",
    POINTERLEAVE = "pointerleave": "record",
    POINTERCANCEL = "pointercancel": "record",
    CLICK = "click": "record",
    DBLCLICK = "dblclick": "record",
    CONTEXTMENU = "contextmenu": "record",
    WHEEL = "wheel": "record",
    KEYDOWN = "keydown": "record",
    KEYUP = "keyup": "record",
    FOCUS = "focus": "void",
    BLUR = "blur": "void",
    FOCUSIN = "focusin": "void",
    FOCUSOUT = "focusout": "void",
    SCROLL = "scroll": "record",
    TRANSITIONEND = "transitionend": "record",
    ANIMATIONEND = "animationend": "record",
    DRAG_BEGIN = "drag-begin": "void",
    DRAG_END = "drag-end": "record",
    DRAG_ENTER = "drag-enter": "string",
    DRAG_LEAVE = "drag-leave": "void",
    DROP = "drop": "record",
);

/// The spec of a common prop.
pub fn prop(id: PropId) -> Option<&'static PropSpec> {
    (0..COMMON.props.len()).find(|&i| COMMON.prop_id(i) == id).map(|i| &COMMON.props[i])
}

/// The spec of a common signal.
pub fn signal(id: SignalId) -> Option<&'static SignalSpec> {
    (0..COMMON.signals.len()).find(|&i| COMMON.signal_id(i) == id).map(|i| &COMMON.signals[i])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consts_match_the_table() {
        for i in 0..COMMON.props.len() {
            assert!(prop(COMMON.prop_id(i)).is_some());
        }
        for id in [TABINDEX, DISABLED, HIDDEN, ARIA_LABEL, ARIA_LIVE, ARIA_BUSY, TITLE, AUTOFOCUS, FOCUS_SCOPE, ANCHOR, ANCHOR_SCOPE, INVALID, DIALOG_ROLE, STATUS_TIP, STATUS_SECTION, DRAG_DATA, DRAG_TYPE, DROP_TYPES] {
            assert!(prop(id).is_some(), "{id:?}");
        }
        for id in [POINTERDOWN, CLICK, WHEEL, KEYDOWN, FOCUS, FOCUSIN, SCROLL, ANIMATIONEND, DRAG_BEGIN, DRAG_END, DRAG_ENTER, DRAG_LEAVE, DROP] {
            assert!(signal(id).is_some(), "{id:?}");
        }
    }
}
