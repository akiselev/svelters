//! `var()` substitution over cssparser tokens (CSS Variables 1 §3).
//!
//! Substitution is textual over tokens: everything that isn't a `var()` is copied from the source,
//! a `var()` becomes the referenced value (or its fallback) with a space on each side so tokens
//! never paste together. A reference to an unset property with no fallback fails the whole value,
//! which the cascade treats as "invalid at computed-value time" (the property is unset).

use cssparser::{ParseError, Parser, Token};

/// The longest value a substitution may produce; deeper chains are treated as invalid, which
/// stops `--a: var(--b) var(--b)` doubling attacks.
const LIMIT: usize = 64 * 1024;

/// Whether `text` contains a `var()` call anywhere.
pub(crate) fn has_var(text: &str) -> bool {
    fn scan(input: &mut Parser<'_>) -> bool {
        while let Ok(t) = input.next() {
            match t {
                Token::Function(name) if name.eq_ignore_ascii_case("var") => return true,
                Token::Function(_) | Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                    if input.parse_nested_block(|i| Ok::<_, cssparser::ParseError<()>>(scan(i))).unwrap_or(false) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }
    // fast path: the tokenizer only runs when the text could contain one
    text.as_bytes().windows(4).any(|w| w.eq_ignore_ascii_case(b"var(")) && scan(&mut Parser::new(text))
}

/// The custom property names `text` references (without fallbacks' own references).
pub(crate) fn references(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let _ = substitute(text, &mut |name| {
        out.push(name.to_string());
        Some(String::new())
    });
    out
}

/// Substitutes every `var()` in `text`. `lookup` returns a custom property's computed value.
pub(crate) fn substitute(text: &str, lookup: &mut dyn FnMut(&str) -> Option<String>) -> Result<String, ()> {
    let mut out = String::with_capacity(text.len());
    let mut input = Parser::new(text);
    walk(&mut input, &mut out, lookup)?;
    Ok(out.trim().to_string())
}

fn walk(input: &mut Parser<'_>, out: &mut String, lookup: &mut dyn FnMut(&str) -> Option<String>) -> Result<(), ()> {
    loop {
        let start = input.position();
        let tok = match input.next_including_whitespace() {
            Ok(t) => t.clone(),
            Err(_) => return Ok(()),
        };
        match tok {
            Token::Function(ref name) if name.eq_ignore_ascii_case("var") => {
                let value = input
                    .parse_nested_block(|i| {
                        let name = i.expect_ident_cloned().map_err(|_| ParseError::<()>::custom(()))?;
                        if !name.starts_with("--") {
                            return Err(ParseError::custom(()));
                        }
                        let has_fallback = i.try_parse(|i| i.expect_comma()).is_ok();
                        match lookup(&name) {
                            Some(v) => {
                                while i.next().is_ok() {}
                                Ok(v)
                            }
                            None if has_fallback => {
                                let mut fb = String::new();
                                walk(i, &mut fb, lookup).map_err(|_| ParseError::custom(()))?;
                                Ok(fb.trim().to_string())
                            }
                            None => Err(ParseError::custom(())),
                        }
                    })
                    .map_err(|_| ())?;
                out.push(' ');
                out.push_str(&value);
                out.push(' ');
            }
            Token::Function(_) | Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                let close = match tok {
                    Token::SquareBracketBlock => ']',
                    Token::CurlyBracketBlock => '}',
                    _ => ')',
                };
                out.push_str(input.slice_from(start));
                input.parse_nested_block(|i| walk(i, out, lookup).map_err(|_| ParseError::<()>::custom(()))).map_err(|_| ())?;
                out.push(close);
            }
            _ => out.push_str(input.slice_from(start)),
        }
        if out.len() > LIMIT {
            return Err(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(text: &str, vars: &[(&str, &str)]) -> Result<String, ()> {
        substitute(text, &mut |n| vars.iter().find(|(k, _)| *k == n).map(|(_, v)| v.to_string()))
    }

    #[test]
    fn substitutes() {
        assert!(has_var("calc(var(--a) + 1px)"));
        assert!(has_var("VAR(--a)"));
        assert!(!has_var("'var(--a)'"));
        assert!(!has_var("12px"));
        assert_eq!(sub("var(--a)", &[("--a", "red")]), Ok("red".into()));
        assert_eq!(sub("calc(var(--a) + 1px)", &[("--a", "2px")]), Ok("calc( 2px  + 1px)".into()));
        assert_eq!(sub("var(--a)px", &[("--a", "2")]), Ok("2 px".into()));
        assert_eq!(sub("var(--missing, 3px)", &[]), Ok("3px".into()));
        assert_eq!(sub("var(--missing, var(--b, 4px))", &[]), Ok("4px".into()));
        assert_eq!(sub("var(--missing,)", &[]), Ok("".into()));
        assert_eq!(sub("var(--missing)", &[]), Err(()));
        assert_eq!(sub("var(a)", &[]), Err(()));
        assert_eq!(sub("1px 'var(--a)'", &[]), Ok("1px 'var(--a)'".into()));
        assert_eq!(references("var(--a) var(--b, var(--c))"), vec!["--a".to_string(), "--b".to_string()]);
    }
}
