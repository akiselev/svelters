//! Template parser for the supported `.svelte` subset (see docs/notes/native-compiler/language-subset.md).
//!
//! Deliberate departures from Svelte's HTML parser: element names are native element names, so no
//! HTML rules apply (no void elements, no implicitly closed `<p>`, no SVG/MathML namespaces by tag
//! name). JavaScript is not parsed here: expressions are located by a JS-aware brace scanner and
//! recorded as byte spans; `js.rs` parses them with oxc.

use crate::diag::Diagnostics;

pub type Span = (u32, u32);

#[derive(Debug)]
pub struct Component {
    pub instance: Option<Block>,
    pub style: Option<Block>,
    pub fragment: Vec<Node>,
}

/// A `<script>` or `<style>` block: `content` is the span between the tags.
#[derive(Debug)]
pub struct Block {
    pub span: Span,
    pub content: Span,
    pub attrs: Vec<Attr>,
}

#[derive(Debug)]
pub enum Node {
    Element(Element),
    Text(Text),
    Expr(Span),
    If(IfBlock),
    Each(EachBlock),
}

#[derive(Debug)]
pub struct Element {
    pub name: String,
    pub name_span: Span,
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub children: Vec<Node>,
    /// Capitalised or dotted tag: a component reference, not a native element.
    pub is_component: bool,
}

#[derive(Debug)]
pub struct Text {
    /// Entity-decoded text.
    pub value: String,
    pub span: Span,
}

#[derive(Debug)]
pub struct IfBlock {
    pub span: Span,
    pub test: Span,
    pub consequent: Vec<Node>,
    /// `{:else}` body; an `{:else if}` chain is a single nested `If` node.
    pub alternate: Option<Vec<Node>>,
}

#[derive(Debug)]
pub struct EachBlock {
    pub span: Span,
    pub expr: Span,
    /// The item binding: only a plain identifier is supported.
    pub context: Span,
    pub index: Option<Span>,
    pub key: Option<Span>,
    pub body: Vec<Node>,
    pub fallback: Option<Vec<Node>>,
}

#[derive(Debug)]
pub struct Attr {
    pub span: Span,
    pub kind: AttrKind,
}

#[derive(Debug)]
pub enum AttrKind {
    /// `name` (bare) or `name="text"`: `value` is None for a bare attribute.
    Static { name: String, name_span: Span, value: Option<String> },
    /// `name={expr}` or the `{name}` shorthand (`expr` is then the identifier itself).
    Expr { name: String, name_span: Span, expr: Span },
    /// `name="a {b} c"`.
    Interp { name: String, name_span: Span, parts: Vec<Part> },
    /// `kind:name` or `kind:name={expr}`; `expr` None means the shorthand `bind:text` == `bind:text={text}`.
    Directive { kind: String, name: String, name_span: Span, expr: Option<Span> },
    Spread { expr: Span },
}

#[derive(Debug)]
pub enum Part {
    Text(String),
    Expr(Span),
}

pub fn parse(src: &str, diags: &mut Diagnostics) -> Option<Component> {
    let mut p = Parser { src, b: src.as_bytes(), pos: 0, diags };
    let mut instance = None;
    let mut style = None;
    let fragment = p.fragment(&mut |p, name, start| {
        // top-level <script>/<style> hook
        match name {
            "script" | "style" => {
                let attrs = p.attrs()?;
                if !p.eat(">") {
                    p.err("expected_gt", "expected `>`");
                    return None;
                }
                let content_start = p.pos;
                let close = format!("</{name}>");
                let Some(rel) = src[p.pos..].find(&close) else {
                    p.err("unclosed_block", format!("<{name}> is not closed"));
                    return None;
                };
                let content = (content_start as u32, (p.pos + rel) as u32);
                p.pos += rel + close.len();
                let block = Block { span: (start as u32, p.pos as u32), content, attrs };
                if name == "script" {
                    if block.attrs.iter().any(|a| matches!(&a.kind, AttrKind::Static { name, .. } if name == "module" || name == "context")) {
                        p.diags.error("unsupported_module_script", block.span, "<script module> is not supported by the native compiler yet");
                    } else if instance.is_some() {
                        p.diags.error("duplicate_script", block.span, "a component can have only one instance <script>");
                    } else {
                        instance = Some(block);
                    }
                } else if style.is_some() {
                    p.diags.error("duplicate_style", block.span, "a component can have only one <style>");
                } else {
                    style = Some(block);
                }
                Some(true)
            }
            _ => Some(false),
        }
    })?;
    if p.pos < src.len() {
        p.err("unexpected_close", "unexpected closing tag or block");
        return None;
    }
    Some(Component { instance, style, fragment })
}

type TopHook<'h, 's, 'd> = dyn FnMut(&mut Parser<'s, 'd>, &str, usize) -> Option<bool> + 'h;

struct Parser<'s, 'd> {
    src: &'s str,
    b: &'s [u8],
    pos: usize,
    diags: &'d mut Diagnostics,
}

impl<'s, 'd> Parser<'s, 'd> {
    fn err(&mut self, code: &'static str, msg: impl Into<String>) {
        let p = self.pos as u32;
        self.diags.error(code, (p, p + 1), msg);
    }

    fn at(&self, s: &str) -> bool {
        self.src[self.pos..].starts_with(s)
    }

    fn eat(&mut self, s: &str) -> bool {
        if self.at(s) {
            self.pos += s.len();
            true
        } else {
            false
        }
    }

    fn ws(&mut self) {
        while self.pos < self.b.len() && self.b[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn ident(&mut self) -> &'s str {
        let start = self.pos;
        while self.pos < self.b.len() {
            let c = self.b[self.pos];
            if c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'$' | b'.' | b':') {
                self.pos += 1;
            } else {
                break;
            }
        }
        &self.src[start..self.pos]
    }

    /// Parses nodes until a closing tag `</`, a block continuation `{:`/`{/`, or EOF.
    /// `top` is only passed at the root, where it handles `<script>`/`<style>`.
    fn fragment(&mut self, top: &mut TopHook<'_, 's, 'd>) -> Option<Vec<Node>> {
        let mut out = Vec::new();
        while self.pos < self.b.len() {
            if self.at("</") || self.at("{:") || self.at("{/") {
                break;
            }
            if self.at("<!--") {
                match self.src[self.pos..].find("-->") {
                    Some(rel) => self.pos += rel + 3,
                    None => {
                        self.err("unclosed_comment", "comment is not closed");
                        return None;
                    }
                }
            } else if self.at("<") {
                let start = self.pos;
                self.pos += 1;
                let name_start = self.pos;
                let name = self.ident();
                if name.is_empty() {
                    self.err("expected_tag", "expected a tag name");
                    return None;
                }
                if top(self, name, start)? {
                    continue;
                }
                let name_span = (name_start as u32, self.pos as u32);
                out.push(Node::Element(self.element(name, start, name_span)?));
            } else if self.at("{#") {
                out.push(self.block()?);
            } else if self.at("{@") {
                let start = self.pos;
                let end = scan_expr_end(self.b, self.pos + 1).unwrap_or(self.b.len());
                self.diags.error(
                    "unsupported_tag",
                    (start as u32, end as u32 + 1),
                    format!("`{}` tags are not supported by the native compiler yet", self.src[start + 1..].split(|c: char| c.is_whitespace() || c == '}').next().unwrap_or("")),
                );
                self.pos = (end + 1).min(self.b.len());
            } else if self.at("{") {
                let start = self.pos + 1;
                let Some(end) = scan_expr_end(self.b, start) else {
                    self.err("unclosed_expr", "expression tag is not closed");
                    return None;
                };
                out.push(Node::Expr((start as u32, end as u32)));
                self.pos = end + 1;
            } else {
                let start = self.pos;
                while self.pos < self.b.len() && self.b[self.pos] != b'<' && self.b[self.pos] != b'{' {
                    self.pos += 1;
                }
                out.push(Node::Text(Text { value: decode_entities(&self.src[start..self.pos]), span: (start as u32, self.pos as u32) }));
            }
        }
        Some(out)
    }

    fn element(&mut self, name: &str, start: usize, name_span: Span) -> Option<Element> {
        let attrs = self.attrs()?;
        let is_component = name.starts_with(|c: char| c.is_ascii_uppercase()) || name.contains('.');
        let mut children = Vec::new();
        if self.eat("/>") {
        } else if self.eat(">") {
            children = self.fragment(&mut |_, _, _| Some(false))?;
            let close = format!("</{name}");
            if !self.eat(&close) {
                self.diags.error("unclosed_element", (start as u32, name_span.1), format!("<{name}> is not closed"));
                return None;
            }
            self.ws();
            if !self.eat(">") {
                self.err("expected_gt", "expected `>`");
                return None;
            }
        } else {
            self.err("expected_gt", "expected `>` or `/>`");
            return None;
        }
        Some(Element { name: name.to_string(), name_span, span: (start as u32, self.pos as u32), attrs, children, is_component })
    }

    fn attrs(&mut self) -> Option<Vec<Attr>> {
        let mut out = Vec::new();
        loop {
            self.ws();
            if self.at(">") || self.at("/>") || self.pos >= self.b.len() {
                return Some(out);
            }
            let start = self.pos;
            if self.at("{") {
                let e0 = self.pos + 1;
                let Some(e1) = scan_expr_end(self.b, e0) else {
                    self.err("unclosed_expr", "attribute expression is not closed");
                    return None;
                };
                self.pos = e1 + 1;
                let inner = self.src[e0..e1].trim();
                let lead = self.src[e0..e1].len() - self.src[e0..e1].trim_start().len();
                let span = (start as u32, self.pos as u32);
                if let Some(rest) = inner.strip_prefix("...") {
                    let s = e0 + lead + 3;
                    out.push(Attr { span, kind: AttrKind::Spread { expr: (s as u32, (s + rest.len()) as u32) } });
                } else {
                    let s = (e0 + lead) as u32;
                    let e = s + inner.len() as u32;
                    out.push(Attr { span, kind: AttrKind::Expr { name: inner.to_string(), name_span: (s, e), expr: (s, e) } });
                }
                continue;
            }
            let name = self.ident().to_string();
            if name.is_empty() {
                self.err("expected_attr", "expected an attribute");
                return None;
            }
            let name_span = (start as u32, self.pos as u32);
            let (dir, dname) = match name.split_once(':') {
                Some((k, n)) if matches!(k, "bind" | "class" | "on" | "use" | "transition" | "in" | "out" | "animate" | "style" | "let") => (Some(k.to_string()), n.to_string()),
                _ => (None, name.clone()),
            };
            self.ws();
            let value = if self.eat("=") {
                self.ws();
                Some(self.attr_value()?)
            } else {
                None
            };
            let span = (start as u32, self.pos as u32);
            let kind = match (dir, value) {
                (Some(kind), None) => AttrKind::Directive { kind, name: dname, name_span, expr: None },
                (Some(kind), Some(AttrValue::Expr(e))) => AttrKind::Directive { kind, name: dname, name_span, expr: Some(e) },
                (Some(kind), Some(_)) => {
                    self.diags.error("directive_value", span, format!("`{kind}:{dname}` needs an expression value: {kind}:{dname}={{...}}"));
                    continue;
                }
                (None, None) => AttrKind::Static { name, name_span, value: None },
                (None, Some(AttrValue::Text(t))) => AttrKind::Static { name, name_span, value: Some(t) },
                (None, Some(AttrValue::Expr(e))) => AttrKind::Expr { name, name_span, expr: e },
                (None, Some(AttrValue::Parts(parts))) => AttrKind::Interp { name, name_span, parts },
            };
            out.push(Attr { span, kind });
        }
    }

    fn attr_value(&mut self) -> Option<AttrValue> {
        if self.at("{") {
            let e0 = self.pos + 1;
            let Some(e1) = scan_expr_end(self.b, e0) else {
                self.err("unclosed_expr", "attribute expression is not closed");
                return None;
            };
            self.pos = e1 + 1;
            return Some(AttrValue::Expr((e0 as u32, e1 as u32)));
        }
        let quote = match self.b.get(self.pos) {
            Some(b'"') => Some(b'"'),
            Some(b'\'') => Some(b'\''),
            _ => None,
        };
        if quote.is_some() {
            self.pos += 1;
        }
        let mut parts = Vec::new();
        let mut text_start = self.pos;
        loop {
            let Some(&c) = self.b.get(self.pos) else {
                self.err("unclosed_attr", "attribute value is not closed");
                return None;
            };
            let end = match quote {
                Some(q) => c == q,
                None => c.is_ascii_whitespace() || c == b'>' || self.at("/>"),
            };
            if end || c == b'{' {
                if self.pos > text_start {
                    parts.push(Part::Text(decode_entities(&self.src[text_start..self.pos])));
                }
                if end {
                    break;
                }
                let e0 = self.pos + 1;
                let Some(e1) = scan_expr_end(self.b, e0) else {
                    self.err("unclosed_expr", "attribute expression is not closed");
                    return None;
                };
                parts.push(Part::Expr((e0 as u32, e1 as u32)));
                self.pos = e1 + 1;
                text_start = self.pos;
                continue;
            }
            self.pos += 1;
        }
        if quote.is_some() {
            self.pos += 1;
        }
        Some(match parts.as_slice() {
            [] => AttrValue::Text(String::new()),
            [Part::Text(_)] => match parts.pop() {
                Some(Part::Text(t)) => AttrValue::Text(t),
                _ => unreachable!(),
            },
            [Part::Expr(e)] if quote.is_none() => AttrValue::Expr(*e),
            _ => AttrValue::Parts(parts),
        })
    }

    /// `{#if ...}` / `{#each ...}`; the parser is positioned at `{#`.
    fn block(&mut self) -> Option<Node> {
        let start = self.pos;
        self.pos += 2;
        let kw = self.ident().to_string();
        let Some(close) = scan_expr_end(self.b, self.pos) else {
            self.err("unclosed_block", "block tag is not closed");
            return None;
        };
        let head = (self.pos as u32, close as u32);
        self.pos = close + 1;
        match kw.as_str() {
            "if" => self.if_rest(start, trim_span(self.src, head)),
            "each" => {
                let (expr, context, index, key) = self.each_head(head)?;
                let body = self.fragment(&mut |_, _, _| Some(false))?;
                let mut fallback = None;
                if self.at("{:else}") {
                    self.pos += 7;
                    fallback = Some(self.fragment(&mut |_, _, _| Some(false))?);
                }
                self.expect_close("each", start)?;
                Some(Node::Each(EachBlock { span: (start as u32, self.pos as u32), expr, context, index, key, body, fallback }))
            }
            other => {
                self.diags.error("unsupported_block", (start as u32, self.pos as u32), format!("`{{#{other}}}` blocks are not supported by the native compiler yet"));
                None
            }
        }
    }

    fn if_rest(&mut self, start: usize, test: Span) -> Option<Node> {
        let consequent = self.fragment(&mut |_, _, _| Some(false))?;
        let mut alternate = None;
        if self.at("{:else if") {
            let s = self.pos;
            self.pos += "{:else if".len();
            let Some(close) = scan_expr_end(self.b, self.pos) else {
                self.err("unclosed_block", "block tag is not closed");
                return None;
            };
            let t = trim_span(self.src, (self.pos as u32, close as u32));
            self.pos = close + 1;
            // the nested if consumes the shared `{/if}`
            let nested = self.if_rest(s, t)?;
            return Some(Node::If(IfBlock { span: (start as u32, self.pos as u32), test, consequent, alternate: Some(vec![nested]) }));
        }
        if self.at("{:else}") {
            self.pos += 7;
            alternate = Some(self.fragment(&mut |_, _, _| Some(false))?);
        }
        self.expect_close("if", start)?;
        Some(Node::If(IfBlock { span: (start as u32, self.pos as u32), test, consequent, alternate }))
    }

    fn expect_close(&mut self, kw: &str, start: usize) -> Option<()> {
        let close = format!("{{/{kw}}}");
        if self.eat(&close) {
            Some(())
        } else {
            self.diags.error("unclosed_block", (start as u32, start as u32 + 1), format!("`{{#{kw}}}` is not closed with `{close}`"));
            None
        }
    }

    /// `rows as row, i (row.id)` -> spans of (collection, item, index, key).
    fn each_head(&mut self, head: Span) -> Option<(Span, Span, Option<Span>, Option<Span>)> {
        let (h0, h1) = (head.0 as usize, head.1 as usize);
        let text = &self.src[h0..h1];
        let Some(as_at) = find_top_level(text.as_bytes(), b" as ") else {
            self.diags.error("each_syntax", head, "expected `{#each <expression> as <name>}`");
            return None;
        };
        let expr = trim_span(self.src, (head.0, (h0 + as_at) as u32));
        let mut rest = (h0 + as_at + 4, h1);
        let mut key = None;
        let rt = self.src[rest.0..rest.1].trim_end();
        if rt.ends_with(')') {
            let close = rest.0 + rt.len() - 1;
            if let Some(open) = matching_open(self.b, rest.0, close) {
                key = Some(trim_span(self.src, (open as u32 + 1, close as u32)));
                rest.1 = open;
            }
        }
        let ctx_text = &self.src[rest.0..rest.1];
        let (ctx, index) = match find_top_level(ctx_text.as_bytes(), b",") {
            Some(c) => (
                trim_span(self.src, (rest.0 as u32, (rest.0 + c) as u32)),
                Some(trim_span(self.src, ((rest.0 + c + 1) as u32, rest.1 as u32))),
            ),
            None => (trim_span(self.src, (rest.0 as u32, rest.1 as u32)), None),
        };
        Some((expr, ctx, index, key))
    }
}

enum AttrValue {
    Text(String),
    Expr(Span),
    Parts(Vec<Part>),
}

fn trim_span(src: &str, (a, b): Span) -> Span {
    let s = &src[a as usize..b as usize];
    let lead = s.len() - s.trim_start().len();
    let trail = s.len() - s.trim_end().len();
    (a + lead as u32, b - trail as u32)
}

/// Index of `pat` in `s` at bracket depth 0 and outside strings.
fn find_top_level(s: &[u8], pat: &[u8]) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = 0;
    while i < s.len() {
        match s[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'"' | b'\'' | b'`' => {
                i = skip_string(s, i);
                continue;
            }
            _ => {}
        }
        if depth == 0 && s[i..].starts_with(pat) {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn matching_open(b: &[u8], lo: usize, close: usize) -> Option<usize> {
    let mut depth = 0;
    let mut i = close;
    loop {
        match b[i] {
            b')' => depth += 1,
            b'(' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        if i == lo {
            return None;
        }
        i -= 1;
    }
}

/// Skips a string or template literal starting at `i` (the quote); returns the index after it.
fn skip_string(b: &[u8], i: usize) -> usize {
    let q = b[i];
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            c if c == q => return j + 1,
            b'$' if q == b'`' && b.get(j + 1) == Some(&b'{') => {
                j = scan_expr_end(b, j + 2).map_or(b.len(), |e| e + 1);
            }
            _ => j += 1,
        }
    }
    b.len()
}

/// Finds the `}` that closes an expression starting at `start`, skipping strings, template
/// literals, comments, regex literals (heuristically) and nested brackets.
pub fn scan_expr_end(b: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = start;
    let mut prev_sig = b'(';
    while i < b.len() {
        let c = b[i];
        match c {
            b'"' | b'\'' | b'`' => {
                i = skip_string(b, i);
                prev_sig = b'"';
                continue;
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
                continue;
            }
            b'/' if b"(,=:[!&|?{};+-*%<>~^".contains(&prev_sig) => {
                // regex literal
                i += 1;
                let mut class = false;
                while i < b.len() {
                    match b[i] {
                        b'\\' => i += 1,
                        b'[' => class = true,
                        b']' => class = false,
                        b'/' if !class => break,
                        _ => {}
                    }
                    i += 1;
                }
                i += 1;
                prev_sig = b'"';
                continue;
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' => depth -= 1,
            b'}' => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
            _ => {}
        }
        if !c.is_ascii_whitespace() {
            prev_sig = c;
        }
        i += 1;
    }
    None
}

pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest.find(';').filter(|&e| e <= 10);
        let decoded = end.and_then(|e| {
            let ent = &rest[1..e];
            let ch = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                _ if ent.starts_with("#x") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
                _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
                _ => None,
            };
            ch.map(|c| (c, e))
        });
        match decoded {
            Some((c, e)) => {
                out.push(c);
                rest = &rest[e + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
