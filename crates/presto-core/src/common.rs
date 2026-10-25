//! The members every node has, whatever its kind: the common props (focus order, disabled, hidden,
//! the accessible label) and the DOM-style events the document dispatches (pointer, click, wheel,
//! keys, focus). They are written as the pseudo-kind `"*"`, so their ids are
//! `PropId::of("*", name, type)` like any kind's.

use crate::ids::{MethodId, PropId, SignalId};
use crate::kind::{KindSpec, MethodSpec, PropSpec, SignalFlags, SignalSpec};
use crate::value::{PropDefault, ValueType};
use presto_base::change::ChangeClass;

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

pub static COMMON: KindSpec = KindSpec {
    props: &[
        PropSpec::new("tabindex", ValueType::I64, ChangeClass::Paint),
        PropSpec::new("disabled", ValueType::Bool, ChangeClass::Paint).default(PropDefault::Bool(false)),
        PropSpec::new("hidden", ValueType::Bool, ChangeClass::Layout).default(PropDefault::Bool(false)),
        PropSpec::new("aria-label", ValueType::Str, ChangeClass::Paint),
        PropSpec::new("title", ValueType::Str, ChangeClass::Paint),
        PropSpec::new("autofocus", ValueType::Bool, ChangeClass::Paint).default(PropDefault::Bool(false)),
        PropSpec::new("focus-scope", ValueType::Enum, ChangeClass::Paint).default(PropDefault::Enum(0)).enum_values(FOCUS_SCOPES),
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
    ],
    methods: &[
        MethodSpec::new("focus").doc("Moves keyboard focus to the node (when it can take focus)."),
        MethodSpec::new("blur").doc("Gives up keyboard focus, when the node or a node inside it has it."),
        MethodSpec::new("scroll-into-view").doc("Scrolls the node's scroll containers so it is visible."),
        MethodSpec::new("bounds").returns(ValueType::Record, true).doc("The node's border box in window points (`{x, y, width, height}`), `null` outside a window."),
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
    TITLE = "title": "string",
    AUTOFOCUS = "autofocus": "bool",
    FOCUS_SCOPE = "focus-scope": "enum",
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
);

ids!(MethodId:
    FOCUS_METHOD = "focus": "()->void",
    BLUR_METHOD = "blur": "()->void",
    SCROLL_INTO_VIEW = "scroll-into-view": "()->void",
    BOUNDS = "bounds": "()->record?",
);

/// The spec of a common method.
pub fn method(id: MethodId) -> Option<&'static MethodSpec> {
    (0..COMMON.methods.len()).find(|&i| COMMON.method_id(i) == id).map(|i| &COMMON.methods[i])
}

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
        for id in [TABINDEX, DISABLED, HIDDEN, ARIA_LABEL, TITLE, AUTOFOCUS, FOCUS_SCOPE] {
            assert!(prop(id).is_some(), "{id:?}");
        }
        for id in [POINTERDOWN, CLICK, WHEEL, KEYDOWN, FOCUS, FOCUSIN, SCROLL, ANIMATIONEND] {
            assert!(signal(id).is_some(), "{id:?}");
        }
        for id in [FOCUS_METHOD, BLUR_METHOD, SCROLL_INTO_VIEW, BOUNDS] {
            assert!(method(id).is_some(), "{id:?}");
        }
    }
}
