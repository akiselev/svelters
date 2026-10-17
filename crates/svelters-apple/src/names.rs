//! Native name -> JS name rules (pure; the generator in `tools/metadata/appkit/` implements the same
//! rules in Python, and both are tested against the same pairs).
//!
//! - Element tag: the class name without the framework prefix, in kebab-case (`NSStackView` -> `stack-view`).
//! - Props: the ObjC property name in kebab-case (`stringValue` -> `string-value`).
//! - Methods: the selector's keywords joined, in kebab-case (`insertArrangedSubview:atIndex:` ->
//!   `insert-arranged-subview-at-index`).
//! - Notification signals: the notification name without prefix and `Notification` suffix
//!   (`NSControlTextDidChangeNotification` -> `control-text-did-change`).

/// Splits a camelCase / PascalCase identifier into lowercase words joined by `-`.
///
/// A run of capitals is one word, except that its last capital starts the next word when a
/// lowercase letter follows (`URLString` -> `url-string`). Digits stay with the preceding word.
pub fn kebab(ident: &str) -> String {
    let chars: Vec<char> = ident.chars().collect();
    let mut out = String::with_capacity(ident.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if c == '_' || c == ':' || c == '-' {
            if !out.is_empty() && !out.ends_with('-') {
                out.push('-');
            }
            continue;
        }
        if c.is_ascii_uppercase() && i > 0 && !out.is_empty() && !out.ends_with('-') {
            let prev = chars[i - 1];
            let next = chars.get(i + 1).copied();
            let boundary = prev.is_ascii_lowercase()
                || prev.is_ascii_digit()
                || (prev.is_ascii_uppercase() && next.is_some_and(|n| n.is_ascii_lowercase()));
            if boundary {
                out.push('-');
            }
        }
        out.push(c.to_ascii_lowercase());
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// The element tag of a class: `prefix` removed, then kebab-case. `NSButton` -> `button`.
pub fn class_tag(class: &str, prefix: &str) -> String {
    kebab(class.strip_prefix(prefix).unwrap_or(class))
}

/// The JS name of a selector: keywords joined, kebab-case. `makeKeyAndOrderFront:` -> `make-key-and-order-front`.
pub fn selector_name(sel: &str) -> String {
    let mut s = String::with_capacity(sel.len());
    for (i, part) in sel.split(':').filter(|p| !p.is_empty()).enumerate() {
        if i == 0 {
            s.push_str(part);
        } else {
            let mut cs = part.chars();
            if let Some(f) = cs.next() {
                s.push(f.to_ascii_uppercase());
                s.push_str(cs.as_str());
            }
        }
    }
    kebab(&s)
}

/// Number of arguments a selector takes (its colons).
pub fn selector_arity(sel: &str) -> usize {
    sel.bytes().filter(|&b| b == b':').count()
}

/// The JS signal name of a notification: `NSWindowWillCloseNotification` -> `window-will-close`.
pub fn notification_signal(name: &str, prefix: &str) -> String {
    let n = name.strip_prefix(prefix).unwrap_or(name);
    kebab(n.strip_suffix("Notification").unwrap_or(n))
}

/// The ObjC setter selector of a property key: `stringValue` -> `setStringValue:`.
pub fn setter_for(key: &str) -> String {
    let mut cs = key.chars();
    match cs.next() {
        Some(f) => format!("set{}{}:", f.to_ascii_uppercase(), cs.as_str()),
        None => String::new(),
    }
}

/// Method families that return +1 objects or consume the receiver (Cocoa naming convention, as
/// ARC uses it). The bridge refuses to call them dynamically: objects are made with `create`.
pub fn owning_family(sel: &str) -> bool {
    let n = sel.trim_start_matches('_');
    ["alloc", "new", "copy", "mutableCopy", "init"].iter().any(|f| {
        n.strip_prefix(f).is_some_and(|rest| rest.is_empty() || rest.starts_with(|c: char| c.is_ascii_uppercase() || c == ':'))
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn families() {
        assert!(owning_family("new"));
        assert!(owning_family("initWithFrame:"));
        assert!(owning_family("copy"));
        assert!(!owning_family("newline"));
        assert!(!owning_family("initialFirstResponder"));
        assert!(!owning_family("center"));
    }

    use super::*;

    /// The same pairs are checked by `tools/metadata/appkit/test_names.py`.
    pub const PAIRS: &[(&str, &str)] = &[
        ("stringValue", "string-value"),
        ("doubleValue", "double-value"),
        ("URLString", "url-string"),
        ("allowsMixedState", "allows-mixed-state"),
        ("contentViewController", "content-view-controller"),
        ("numberOfTickMarks", "number-of-tick-marks"),
        ("acceptsFirstResponder", "accepts-first-responder"),
        ("toolTip", "tool-tip"),
        ("alphaValue", "alpha-value"),
        ("isHidden", "is-hidden"),
        ("frameRotation", "frame-rotation"),
        ("userInterfaceLayoutDirection", "user-interface-layout-direction"),
        ("NSTextField", "ns-text-field"),
        ("TextField", "text-field"),
        ("maxValue", "max-value"),
        ("int32Value", "int32-value"),
        ("CGColor", "cg-color"),
    ];

    #[test]
    fn kebab_pairs() {
        for (a, b) in PAIRS {
            assert_eq!(kebab(a), *b, "{a}");
        }
    }

    #[test]
    fn tags() {
        assert_eq!(class_tag("NSStackView", "NS"), "stack-view");
        assert_eq!(class_tag("NSButton", "NS"), "button");
        assert_eq!(class_tag("NSTextField", "NS"), "text-field");
        assert_eq!(class_tag("NSWindow", "NS"), "window");
        assert_eq!(class_tag("NSProgressIndicator", "NS"), "progress-indicator");
    }

    #[test]
    fn selectors() {
        assert_eq!(selector_name("makeKeyAndOrderFront:"), "make-key-and-order-front");
        assert_eq!(selector_name("insertArrangedSubview:atIndex:"), "insert-arranged-subview-at-index");
        assert_eq!(selector_name("performClick:"), "perform-click");
        assert_eq!(selector_name("center"), "center");
        assert_eq!(selector_name("setFrameOrigin:"), "set-frame-origin");
        assert_eq!(selector_arity("addSubview:positioned:relativeTo:"), 3);
        assert_eq!(selector_arity("center"), 0);
    }

    #[test]
    fn notifications_and_setters() {
        assert_eq!(notification_signal("NSControlTextDidChangeNotification", "NS"), "control-text-did-change");
        assert_eq!(notification_signal("NSWindowWillCloseNotification", "NS"), "window-will-close");
        assert_eq!(setter_for("stringValue"), "setStringValue:");
        assert_eq!(setter_for("title"), "setTitle:");
    }
}
