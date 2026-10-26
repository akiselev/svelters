//! Shorthand properties (OW-CSS): `outline`, `border`, `border-{top,right,bottom,left}`,
//! `border-{width,style,color}`, `border-radius`, `overflow` and (U2 E5) `transition` expand to longhands at
//! computed-value time, after `var()` substitution, so a later shorthand overrides an earlier
//! longhand and the other way round (CSS Cascade 5 §1.2).
//!
//! `border-width`, `border-color` and `border-radius` are also uniform properties of their own in
//! the egui table (CS5g paints them); one value sets both the uniform property and every side or
//! corner, and a 2–4 value list sets the sides (the uniform property is then invalid, so unset).
//! Read sides and corners through [`StyleView`](crate::StyleView).

use crate::table::Table;
use crate::values::border_width_keyword;

const SIDES: [&str; 4] = ["top", "right", "bottom", "left"];

/// `border-*-style` and `outline-style` keywords (`auto` is outline-only).
pub const LINE_STYLES: &[&str] = &["none", "hidden", "solid", "dashed", "dotted", "double", "groove", "ridge", "inset", "outset"];

const BORDER_WIDTH: [&str; 4] = ["border-top-width", "border-right-width", "border-bottom-width", "border-left-width"];
const BORDER_STYLE: [&str; 4] = ["border-top-style", "border-right-style", "border-bottom-style", "border-left-style"];
const BORDER_COLOR: [&str; 4] = ["border-top-color", "border-right-color", "border-bottom-color", "border-left-color"];
/// The `transition` longhands (U2 E5), in the order [`expand`] returns them.
pub(crate) const TRANSITION: [&str; 4] = ["transition-property", "transition-duration", "transition-timing-function", "transition-delay"];
const BORDER_RADIUS: [&str; 4] =["border-top-left-radius", "border-top-right-radius", "border-bottom-right-radius", "border-bottom-left-radius"];

/// The longhands `name` sets, when it is a shorthand.
pub(crate) fn longhands(name: &str) -> Option<Vec<&'static str>> {
    Some(match name {
        "outline" => vec!["outline-width", "outline-style", "outline-color"],
        "overflow" => vec!["overflow-x", "overflow-y"],
        // the uniform property itself too (egui's table has it; `expand` sets it)
        "border-width" => [&BORDER_WIDTH[..], &["border-width"]].concat(),
        "border-style" => BORDER_STYLE.to_vec(),
        "border-color" => [&BORDER_COLOR[..], &["border-color"]].concat(),
        "border-radius" => [&BORDER_RADIUS[..], &["border-radius"]].concat(),
        // U2 E5: `transition` itself is assembled from these after the cascade (`computed.rs`)
        "transition" => TRANSITION.to_vec(),
        "border" => {
            let mut v = Vec::with_capacity(14);
            for i in 0..4 {
                v.extend([BORDER_WIDTH[i], BORDER_STYLE[i], BORDER_COLOR[i]]);
            }
            v.extend(["border-width", "border-color"]);
            v
        }
        _ => {
            let side = name.strip_prefix("border-").filter(|s| SIDES.contains(s))?;
            let i = SIDES.iter().position(|s| *s == side)?;
            vec![BORDER_WIDTH[i], BORDER_STYLE[i], BORDER_COLOR[i]]
        }
    })
}

/// The longhands of `name` when this engine expands it on `table`: a shorthand with a longhand in
/// the table, and not a property the toolkit's own CSS engine takes whole (GTK's `border`).
pub(crate) fn longhands_in(table: &Table, name: &str) -> Option<Vec<&'static str>> {
    let longs = longhands(name)?;
    if !longs.iter().any(|n| table.index_of(n).is_some()) {
        return None;
    }
    if table.get(name).is_some_and(|p| !table.computes(p)) {
        return None;
    }
    Some(longs)
}

/// Expands `value` (after `var()` substitution) into `(longhand, value text)` pairs.
pub(crate) fn expand(name: &str, value: &str) -> Result<Vec<(&'static str, String)>, String> {
    let longs = longhands(name).ok_or_else(|| format!("`{name}` is not a shorthand"))?;
    let v = value.trim();
    let lower = v.to_ascii_lowercase();
    if matches!(lower.as_str(), "inherit" | "initial" | "unset" | "revert" | "revert-layer") {
        return Ok(longs.into_iter().map(|l| (l, lower.clone())).collect());
    }
    if name == "transition" {
        let texts = crate::values::expand_transition(v)?;
        return Ok(longs.into_iter().zip(texts).collect());
    }
    let parts = split_top(v);
    match name {
        // `<x> [<y>]`: one value sets both axes (CSS Overflow 3 §3)
        "overflow" => match parts.as_slice() {
            [a] => Ok(vec![("overflow-x", a.clone()), ("overflow-y", a.clone())]),
            [a, b] => Ok(vec![("overflow-x", a.clone()), ("overflow-y", b.clone())]),
            _ => Err("overflow: expected 1 or 2 values".into()),
        },
        "border-width" | "border-style" | "border-color" | "border-radius" => {
            if v.contains('/') {
                return Err(format!("{name}: elliptical radii (`/`) are not supported"));
            }
            let four = |p: &[String]| -> Option<[String; 4]> {
                Some(match p {
                    [a] => [a.clone(), a.clone(), a.clone(), a.clone()],
                    [a, b] => [a.clone(), b.clone(), a.clone(), b.clone()],
                    [a, b, c] => [a.clone(), b.clone(), c.clone(), b.clone()],
                    [a, b, c, d] => [a.clone(), b.clone(), c.clone(), d.clone()],
                    _ => return None,
                })
            };
            let vals = four(&parts).ok_or_else(|| format!("{name}: expected 1 to 4 values"))?;
            let mut out: Vec<(&'static str, String)> = longs.iter().copied().zip(vals).collect();
            if name == "border-style"
                && let Some((_, bad)) = out.iter().find(|(_, s)| !LINE_STYLES.contains(&s.to_ascii_lowercase().as_str()))
            {
                return Err(format!("border-style: '{bad}' is not one of {}", LINE_STYLES.join(", ")));
            }
            // the uniform property of the egui table takes the single value only
            if matches!(name, "border-width" | "border-color" | "border-radius") {
                out.push((uniform(name), if parts.len() == 1 { parts[0].clone() } else { "unset".into() }));
            }
            Ok(out)
        }
        _ => {
            // `[width] [style] [color]` in any order
            let (mut width, mut style, mut color) = (None, None, None);
            if parts.len() > 3 {
                return Err(format!("{name}: expected a width, a style and a colour"));
            }
            for p in parts {
                let l = p.to_ascii_lowercase();
                if style.is_none() && (LINE_STYLES.contains(&l.as_str()) || (name == "outline" && l == "auto")) {
                    style = Some(l);
                } else if is_width(&l) {
                    if width.is_some() {
                        return Err(format!("{name}: more than one width"));
                    }
                    width = Some(match border_width_keyword(&l) {
                        Some(px) => format!("{px}px"),
                        None => p,
                    });
                } else if color.is_none() {
                    color = Some(p);
                } else {
                    return Err(format!("{name}: unexpected '{p}'"));
                }
            }
            let width = width.unwrap_or_else(|| "3px".into());
            let style = style.unwrap_or_else(|| "none".into());
            let color = color.unwrap_or_else(|| "currentcolor".into());
            Ok(longs
                .into_iter()
                .map(|l| {
                    let text = if l.ends_with("-width") {
                        width.clone()
                    } else if l.ends_with("-style") {
                        style.clone()
                    } else {
                        color.clone()
                    };
                    (l, text)
                })
                .collect())
        }
    }
}

fn uniform(name: &str) -> &'static str {
    match name {
        "border-width" => "border-width",
        "border-color" => "border-color",
        _ => "border-radius",
    }
}

/// A width token: a number with a unit, `0`, a math function or a width keyword.
fn is_width(t: &str) -> bool {
    if border_width_keyword(t).is_some() || t == "0" {
        return true;
    }
    if ["calc(", "min(", "max(", "clamp("].iter().any(|f| t.starts_with(f)) {
        return true;
    }
    let digits = t.trim_start_matches(['+', '-']);
    digits.starts_with(|c: char| c.is_ascii_digit() || c == '.') && t.chars().any(|c| c.is_ascii_alphabetic())
}

/// Splits on top-level whitespace, keeping `(...)` together.
fn split_top(v: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    for c in v.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if c.is_whitespace() && depth == 0 {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_sides_and_any_order() {
        let e = expand("border-top", "solid 2px rgb(1, 2, 3)").unwrap();
        assert_eq!(e, vec![("border-top-width", "2px".into()), ("border-top-style", "solid".into()), ("border-top-color", "rgb(1, 2, 3)".into())]);
        let e = expand("outline", "thick dashed").unwrap();
        assert_eq!(e, vec![("outline-width", "5px".into()), ("outline-style", "dashed".into()), ("outline-color", "currentcolor".into())]);
        assert!(expand("outline", "1px 2px").is_err());
    }

    #[test]
    fn one_to_four_values() {
        let e = expand("border-width", "1px 2px").unwrap();
        assert_eq!(e[1], ("border-right-width", "2px".into()));
        assert_eq!(e[3], ("border-left-width", "2px".into()));
        assert_eq!(e[4], ("border-width", "unset".into()));
        let e = expand("border-radius", "4px").unwrap();
        assert!(e.iter().all(|(_, v)| v == "4px"));
        assert!(expand("border-style", "solid wavy").is_err());
        assert!(expand("border-radius", "4px / 2px").is_err());
        assert_eq!(expand("border", "inherit").unwrap().len(), 14);
    }

    #[test]
    fn overflow_one_or_two_axes() {
        assert_eq!(expand("overflow", "hidden").unwrap(), vec![("overflow-x", "hidden".into()), ("overflow-y", "hidden".into())]);
        assert_eq!(expand("overflow", "hidden auto").unwrap(), vec![("overflow-x", "hidden".into()), ("overflow-y", "auto".into())]);
        assert!(expand("overflow", "hidden auto scroll").is_err());
        assert_eq!(expand("overflow", "inherit").unwrap().len(), 2);
    }
}
