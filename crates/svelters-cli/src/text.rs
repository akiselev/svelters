//! Doc text clean-up for terminal and JSON output. The IR's text blocks are markdown with
//! `xref:<entity id>#<kind>:<member>` links (resolved to site links by the site renderer), images
//! and some inline HTML. Here links become their text, images are dropped, and allowed inline
//! HTML (`<kbd>`, `<code>`, …) becomes markdown or plain text. Fenced code is left alone.

/// Rewrites doc markdown for the terminal: xref and relative links become their text (external
/// `http(s)` links are kept), images are dropped, a few inline HTML tags are simplified.
pub fn clean_md(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut fence = false;
    for (i, line) in md.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if line.trim_start().starts_with("```") {
            fence = !fence;
            out.push_str(line);
        } else if fence {
            out.push_str(line);
        } else {
            out.push_str(&clean_line(line, true));
        }
    }
    out.trim().to_string()
}

/// Plain text: links become their text, emphasis and code markers and HTML tags are removed.
pub fn plain(md: &str) -> String {
    let s = clean_line(md, false);
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if !matches!(c, '`' | '*') {
            out.push(c);
        }
    }
    out
}

/// One line (or paragraph) of prose. Inline code spans are copied verbatim.
fn clean_line(s: &str, keep_http: bool) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'`' => {
                // inline code span: copy through the closing backtick
                if let Some(end) = s[i + 1..].find('`') {
                    out.push_str(&s[i..i + 1 + end + 1]);
                    i += end + 2;
                    continue;
                }
                out.push('`');
                i += 1;
            }
            b'!' if b.get(i + 1) == Some(&b'[') => {
                if let Some((_, _, next)) = link_at(s, i + 1) {
                    i = next; // drop images
                    continue;
                }
                out.push('!');
                i += 1;
            }
            b'[' => {
                if let Some((text, target, next)) = link_at(s, i) {
                    let inner = clean_line(text, keep_http);
                    if keep_http
                        && (target.starts_with("http://") || target.starts_with("https://"))
                    {
                        out.push('[');
                        out.push_str(&inner);
                        out.push_str("](");
                        out.push_str(target);
                        out.push(')');
                    } else {
                        out.push_str(&inner);
                    }
                    i = next;
                    continue;
                }
                out.push('[');
                i += 1;
            }
            b'<' => {
                // The site's allow-list (render.mjs ALLOWED_HTML); any other tag is shown literally,
                // as the site does (GTK docs mention `<child>` and similar in prose).
                if let Some((name, next)) = tag_at(s, i) {
                    let done = match name.as_str() {
                        "code" | "kbd" => Some("`"),
                        "em" | "i" => Some("*"),
                        "strong" | "b" => Some("**"),
                        "br" => Some(" "),
                        "sub" | "sup" => Some(""),
                        _ => None,
                    };
                    if let Some(rep) = done {
                        out.push_str(rep);
                        i = next;
                        continue;
                    }
                }
                out.push('<');
                i += 1;
            }
            b'&' => {
                let rest = &s[i..];
                let ents = [
                    ("&lt;", '<'),
                    ("&gt;", '>'),
                    ("&amp;", '&'),
                    ("&quot;", '"'),
                    ("&#39;", '\''),
                ];
                if let Some((e, c)) = ents.iter().find(|(e, _)| rest.starts_with(e)) {
                    out.push(*c);
                    i += e.len();
                } else {
                    out.push('&');
                    i += 1;
                }
            }
            _ => {
                let c = s[i..].chars().next().unwrap_or('\u{fffd}');
                out.push(c);
                i += c.len_utf8();
            }
        }
    }
    out
}

/// A markdown link `[text](target)` starting at byte `i` (which holds `[`): (text, target, end).
fn link_at(s: &str, i: usize) -> Option<(&str, &str, usize)> {
    let b = s.as_bytes();
    let mut depth = 0;
    let mut j = i;
    while j < b.len() {
        match b[j] {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            b'\n' => return None,
            _ => {}
        }
        j += 1;
    }
    if j >= b.len() || b.get(j + 1) != Some(&b'(') {
        return None;
    }
    let close = s[j + 2..].find(')')? + j + 2;
    let target = s[j + 2..close].trim();
    if target.contains(char::is_whitespace) {
        return None;
    }
    Some((&s[i + 1..j], target, close + 1))
}

/// An HTML tag `<name …>` / `</name>` at byte `i`: (lower-case name, end).
fn tag_at(s: &str, i: usize) -> Option<(String, usize)> {
    let rest = &s[i + 1..];
    let rest2 = rest.strip_prefix('/').unwrap_or(rest);
    let name: String = rest2
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    if name.is_empty() || !name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let end = rest.find('>')?;
    if rest[..end].contains('<') || rest[..end].contains('\n') {
        return None;
    }
    Some((name.to_ascii_lowercase(), i + 1 + end + 1))
}

/// The first paragraph of a text block, on one line, markdown kept (for member lists).
pub fn first_paragraph(md: &str) -> String {
    let c = clean_md(md);
    let para = c.split("\n\n").find(|p| !p.trim().is_empty()).unwrap_or("");
    para.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xref_links_become_text() {
        assert_eq!(
            clean_md("use the [`Button::clicked`](xref:gtk:Gtk.Button#event:clicked) signal"),
            "use the `Button::clicked` signal"
        );
    }

    #[test]
    fn http_links_kept_images_dropped() {
        assert_eq!(
            clean_md("see [docs](https://x.y/z) ![img](a.png)"),
            "see [docs](https://x.y/z)"
        );
    }

    #[test]
    fn html_simplified() {
        assert_eq!(
            clean_md("press <kbd>Enter</kbd> or <em>this</em>"),
            "press `Enter` or *this*"
        );
        assert_eq!(clean_md("a <child> element"), "a <child> element");
        assert_eq!(clean_md("a &lt;b&gt; c"), "a <b> c");
    }

    #[test]
    fn code_is_untouched() {
        assert_eq!(
            clean_md("`[a](b)` and\n```\n<kbd>x</kbd>\n```"),
            "`[a](b)` and\n```\n<kbd>x</kbd>\n```"
        );
    }

    #[test]
    fn plain_strips_markers() {
        assert_eq!(
            plain("The *dockwidget* `x` [y](xref:qt:Q)"),
            "The dockwidget x y"
        );
    }

    #[test]
    fn first_paragraph_joins_lines() {
        assert_eq!(first_paragraph("one\ntwo\n\nthree"), "one two");
    }
}
