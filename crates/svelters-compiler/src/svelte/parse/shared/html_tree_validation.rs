//! `closing_tag_omitted` of `packages/svelte/src/html-tree-validation.js` (Svelte 5.57.2 @ f3de3f2).

enum Disallowed {
    Direct(&'static [&'static str]),
    /// `reset_by` is not used by `closing_tag_omitted`.
    Descendant(&'static [&'static str]),
}

/// Map of elements that have certain elements that are not allowed inside them, in the sense that they will auto-close the parent/ancestor element.
/// Theoretically one could take advantage of it but most of the time it will just result in confusing behavior and break when SSR'd.
/// There are more elements that are invalid inside other elements, but they're not auto-closed and so don't break SSR and are therefore not listed here.
fn autoclosing_children(name: &str) -> Option<Disallowed> {
    use Disallowed::*;
    Some(match name {
        // based on http://developers.whatwg.org/syntax.html#syntax-tag-omission
        "li" => Direct(&["li"]),
        // https://developer.mozilla.org/en-US/docs/Web/HTML/Element/dt#technical_summary
        "dt" => Descendant(&["dt", "dd"]),
        "dd" => Descendant(&["dt", "dd"]),
        "p" => Descendant(&[
            "address",
            "article",
            "aside",
            "blockquote",
            "div",
            "dl",
            "fieldset",
            "footer",
            "form",
            "h1",
            "h2",
            "h3",
            "h4",
            "h5",
            "h6",
            "header",
            "hgroup",
            "hr",
            "main",
            "menu",
            "nav",
            "ol",
            "p",
            "pre",
            "section",
            "table",
            "ul",
        ]),
        "rt" => Descendant(&["rt", "rp"]),
        "rp" => Descendant(&["rt", "rp"]),
        "optgroup" => Descendant(&["optgroup"]),
        "option" => Descendant(&["option", "optgroup"]),
        "thead" => Direct(&["tbody", "tfoot"]),
        "tbody" => Direct(&["tbody", "tfoot"]),
        "tfoot" => Direct(&["tbody"]),
        "tr" => Direct(&["tr", "tbody"]),
        "td" => Direct(&["td", "th", "tr"]),
        "th" => Direct(&["td", "th", "tr"]),
        _ => return None,
    })
}

/// Returns true if the tag is either the last in the list of siblings and will be autoclosed,
/// or not allowed inside the parent tag such that it will auto-close it.
pub fn closing_tag_omitted(current: &str, next: Option<&str>) -> bool {
    if let Some(disallowed) = autoclosing_children(current) {
        let list = match disallowed {
            Disallowed::Direct(l) | Disallowed::Descendant(l) => l,
        };
        match next {
            None => return true,
            Some(next) if list.contains(&next) => return true,
            _ => {}
        }
    }
    false
}
