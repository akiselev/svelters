//! FW1: `on:` directives in runes mode (ARCH-EMBED §8.7, CADATOMIC SV9 per C-2).
//!
//! - `on:x={h}` on an element is a listener, as `onx={h}` (`lower/mod.rs`); on a component it is
//!   the prop `onx={h}`.
//! - A bare `on:x` inside a component `C` forwards `C`'s own callback prop: on an element it is
//!   the listener `onx={$$props.onx}` (`Handler::Forward`, `SpreadEntry::Forward`), on a child
//!   component the prop `onx={$$props.onx}` (`ComponentProp::Forward`). `$$props` is the
//!   compiler's own props parameter (user code cannot name it in runes mode). There are no legacy
//!   component events: one listener, the parent's, gets the event.
//! - Diagnostics: `legacy_forwarding_lowered` (a note, once per component), `duplicate_event`
//!   (a bare `on:x` together with another handler of `x`; two props `onx` on a component, one of
//!   them from `on:`), and `unsupported_feature` with a fix-it for modifiers (`on:x|once`).

use super::*;
use crate::diag::{FixIt, TextEdit};
use crate::svelte::ast::Directive;

/// Where an `on:` directive is: an element (`<button>`, `<svelte:element>`) or a component.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum OnSite<'t> {
    Element(&'t str),
    Component(&'t str),
}

impl OnSite<'_> {
    fn what(&self) -> String {
        match self {
            OnSite::Element(tag) => format!("<{tag}>"),
            OnSite::Component(name) => format!("<{name}>"),
        }
    }
}

fn directive_span(d: &Directive) -> (u32, u32) {
    d.name_span.map_or(sp(d.span()), |s| (d.start, s.end))
}

impl<'a> Lower<'a, '_, '_> {
    /// A bare `on:x` (no handler, no modifiers): the note, once per component.
    pub(super) fn note_forwarding(&mut self, d: &Directive, site: OnSite) {
        if self.forwarding_noted {
            return;
        }
        self.forwarding_noted = true;
        let key = format!("on{}", d.name);
        let typing = match site {
            OnSite::Element(tag) => format!("Forward<'{tag}', '{}'>", d.name),
            OnSite::Component(name) => format!("ForwardFrom<typeof {name}, '{}'>", d.name),
        };
        self.diags
            .note(
                "legacy_forwarding_lowered",
                directive_span(d),
                format!(
                    "`on:{0}` forwards this component's `{key}` callback prop: the one handler its parent passes gets the event (legacy forwarding re-dispatched it to every listener; svelters has no component events)",
                    d.name
                ),
            )
            .help(format!(
                "for svelte-check, declare the prop in this component's `$props()` type, e.g. `Props & {typing}` (types from `@svelters/native-runtime/types`)"
            ));
    }

    /// `duplicate_event`: on one element, a bare `on:x` must be the only handler of `x`
    /// (`onx={h}`, `on:x={h}` or a second bare `on:x` are reported); on a component, two props
    /// `onx` where one comes from an `on:` directive are reported (the later one would win).
    pub(super) fn check_duplicate_events(&mut self, attributes: &[AttributeLike], site: OnSite) {
        // (key, span, bare, from a directive) in source order
        let mut seen: Vec<(String, (u32, u32), bool, bool)> = Vec::new();
        for a in attributes {
            match a {
                AttributeLike::Attribute(at) if at.name.starts_with("on") && at.name.len() > 2 => {
                    let is_handler = matches!(at.value, AttributeValue::Expression(_));
                    if is_handler || matches!(site, OnSite::Component(_)) {
                        seen.push((at.name.clone(), name_span(at), false, false));
                    }
                }
                AttributeLike::OnDirective(d) if d.modifiers.is_empty() => {
                    seen.push((format!("on{}", d.name), directive_span(d), d.expression.is_none(), true));
                }
                _ => {}
            }
        }
        let mut keys: Vec<&str> = Vec::new();
        for (k, ..) in &seen {
            if !keys.contains(&k.as_str()) {
                keys.push(k);
            }
        }
        for key in keys {
            let all: Vec<&(String, (u32, u32), bool, bool)> = seen.iter().filter(|o| o.0 == key).collect();
            if all.len() < 2 {
                continue;
            }
            let event = &key[2..];
            let what = site.what();
            if let Some(bare) = all.iter().find(|o| o.2) {
                let also = match site {
                    OnSite::Element(_) => format!("{what} also handles `{event}`"),
                    OnSite::Component(_) => format!("{what} also gets the prop `{key}`"),
                };
                self.diags
                    .error(
                        "duplicate_event",
                        bare.1,
                        format!("`on:{event}` forwards this component's `{key}` callback prop, and {also}: a bare `on:{event}` must be its only handler"),
                    )
                    .help(format!(
                        "to handle `{event}` here and forward it, destructure the prop (`let {{ {key} }} = $props()`) and call it from one handler: `{key}={{(e) => {{ handle(e); {key}?.(e); }}}}`"
                    ));
            } else if matches!(site, OnSite::Component(_)) && all.iter().any(|o| o.3) {
                // on a component `on:x={h}` is the prop `onx`: a second one replaces the first
                let last = all.last().unwrap();
                self.diags
                    .error("duplicate_event", last.1, format!("{what} gets the `{key}` prop twice (`on:{event}={{…}}` is the prop `{key}`): only the last one would be passed"))
                    .help(format!("pass one `{key}={{…}}` that does both"));
            }
        }
    }

    /// `on:x|modifier`: not supported in runes mode; the fix-it is the event attribute (or prop),
    /// with what each modifier did written in the handler.
    pub(super) fn on_modifiers(&mut self, d: &Directive, site: OnSite) {
        let key = format!("on{}", d.name);
        let mods = d.modifiers.iter().map(|m| format!("|{m}")).collect::<String>();
        let capture = matches!(site, OnSite::Element(_)) && d.modifiers.iter().any(|m| m == "capture");
        let attr = if capture { format!("{key}capture") } else { key.clone() };
        // the call of the original handler: its name when it is one, else a placeholder; a bare
        // `on:x` calls the destructured prop
        let call = match &d.expression {
            Some(Expression::Identifier(id)) => format!("{}(event)", id.name),
            Some(_) => "handler(event)".to_string(),
            None => format!("{key}?.(event)"),
        };
        let value = match &d.expression {
            Some(Expression::Identifier(id)) => id.name.clone(),
            Some(_) => "handler".to_string(),
            None => key.clone(),
        };
        let mut stmts: Vec<&str> = Vec::new();
        let mut notes: Vec<String> = Vec::new();
        for m in &d.modifiers {
            match m.as_str() {
                "preventDefault" => stmts.push("event.preventDefault();"),
                "stopPropagation" => stmts.push("event.stopPropagation();"),
                "stopImmediatePropagation" => stmts.push("event.stopImmediatePropagation();"),
                "self" => stmts.push("if (event.target !== event.currentTarget) return;"),
                "trusted" => stmts.push("if (!event.isTrusted) return;"),
                "once" => notes.push("ignore the calls after the first (`|once`)".to_string()),
                "capture" if capture => {}
                "passive" | "nonpassive" => {}
                other => notes.push(format!("do what `|{other}` did")),
            }
        }
        let attr_value = if stmts.is_empty() { value } else { format!("(event) => {{ {} {call}; }}", stmts.join(" ")) };
        let mut help = String::new();
        if d.expression.is_none() {
            help.push_str(&format!("destructure the prop (`let {{ {key} }} = $props()`), then "));
        }
        help.push_str(match site {
            OnSite::Element(_) => "write ",
            OnSite::Component(_) => "pass ",
        });
        help.push_str(&format!("`{attr}={{{attr_value}}}`"));
        if !notes.is_empty() {
            help.push_str(&format!(" and, in the handler, {}", notes.join(", ")));
        }
        let on = match site {
            OnSite::Element(_) => "",
            OnSite::Component(_) => " on a component",
        };
        // the fix-it (CX1): the rewrite of the help, when it is exact (a handler is given and no
        // modifier is left for the author to reimplement, and no capture phase, which the backend
        // may not have); the handler's own text stays in place
        let fix = match &d.expression {
            Some(e) if notes.is_empty() && !capture => {
                let (ds, de) = sp(d.span());
                let (es, ee) = (e.span().start, e.span().end);
                let edits = match e {
                    Expression::Identifier(_) if !stmts.is_empty() => vec![TextEdit { start: ds, end: de, text: format!("{attr}={{{attr_value}}}") }],
                    _ if stmts.is_empty() => vec![TextEdit { start: ds, end: es, text: format!("{attr}={{") }, TextEdit { start: ee, end: de, text: "}".into() }],
                    _ => vec![
                        TextEdit { start: ds, end: es, text: format!("{attr}={{(event) => {{ {} (", stmts.join(" ")) },
                        TextEdit { start: ee, end: de, text: ")(event); }}".into() },
                    ],
                };
                Some(FixIt::new(format!("rewrite as `{attr}={{…}}` without modifiers"), edits))
            }
            _ => None,
        };
        let diag = self
            .diags
            .error(
                "unsupported_feature",
                sp(d.span()),
                format!("`on:{}{mods}`: event modifiers{on} are not supported by the native compiler (runes mode has no modifiers)", d.name),
            )
            .help(help);
        if let Some(f) = fix {
            diag.fix(f);
        }
    }
}
