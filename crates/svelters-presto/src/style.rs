//! The `style` host op on presto: the CSS engine's inputs (sheets, classes, id, inline style, colour
//! scheme) go to the tree, which runs svelters-style's cascade (architecture §5); the TR1 visual
//! channels (opacity, transform, clip, inert) are property-class writes (§4.3). presto-core takes
//! sheets and per-node inputs today; the colour scheme and the visual channels are kept by the
//! backend ([`PrestoBackend::color_scheme`], [`PrestoBackend::visual`]) until it takes them.
//!
//! `SVL_STYLE_ANIMATE` answers 0, so the runtime plays the animation itself through the visual
//! channels, until presto-motion's native animations are wired here (lane MOTION-CSS).

use crate::backend::PrestoBackend;
use crate::surface::{ColorScheme, Visual};
use presto_core::StyleInput;
use std::sync::Arc;
use svelters_abi::*;

/// `SVL_STYLE_TARGET`'s answer: the backend runs the Rust CSS engine itself (as egui, CS5e).
pub const SVL_STYLE_TARGET_ENGINE: i32 = 4;

impl PrestoBackend {
    pub(crate) fn style_op(&self, op: u32, h: svl_handle, text: Option<&str>) -> StrResult<i32> {
        match op {
            SVL_STYLE_TARGET => return Ok(SVL_STYLE_TARGET_ENGINE),
            SVL_STYLE_ANIMATE => return Ok(0),
            SVL_STYLE_SHEET => {
                // `id SEP css [SEP file SEP map]`; a bare id removes that sheet, none clears them all
                match text {
                    None => {
                        let all = std::mem::take(&mut *self.0.sheets.borrow_mut());
                        for (_, (id, _)) in all {
                            self.0.doc.borrow_mut().remove_sheet(id);
                        }
                    }
                    Some(t) => match t.split_once(SVL_STYLE_SEP) {
                        Some((name, rest)) => {
                            let css = rest.split_once(SVL_STYLE_SEP).map_or(rest, |(css, _)| css);
                            let mut doc = self.0.doc.borrow_mut();
                            let id = doc.add_sheet(css);
                            if let Some((old, _)) = self.0.sheets.borrow_mut().insert(name.to_string(), (id, css.to_string())) {
                                doc.remove_sheet(old);
                            }
                        }
                        None => {
                            if let Some((old, _)) = self.0.sheets.borrow_mut().remove(t) {
                                self.0.doc.borrow_mut().remove_sheet(old);
                            }
                        }
                    },
                }
                return Ok(0);
            }
            SVL_STYLE_SCHEME => {
                // kept until presto-core evaluates `prefers-color-scheme` from its media state
                let scheme = text.and_then(|t| t.split(SVL_STYLE_SEP).next()).map(|s| if s == "dark" { ColorScheme::Dark } else { ColorScheme::Light });
                self.0.scheme.set(scheme);
                return Ok(0);
            }
            SVL_STYLE_DECLS => return Err("presto runs the CSS engine (style target 4): it takes sheets, not declaration rules".into()),
            _ => {}
        }
        let (n, _) = self.live(h)?;
        match op {
            SVL_STYLE_CLASSES | SVL_STYLE_NAME | SVL_STYLE_INLINE => {
                let mut styles = self.0.styles.borrow_mut();
                let s: &mut StyleInput = styles.entry(n).or_default();
                match op {
                    SVL_STYLE_CLASSES => {
                        s.classes.clear();
                        for c in text.unwrap_or("").split_whitespace() {
                            if !s.classes.iter().any(|x| &**x == c) {
                                s.classes.push(Arc::from(c));
                            }
                        }
                    }
                    SVL_STYLE_NAME => s.id = text.filter(|t| !t.is_empty()).map(Arc::from),
                    _ => s.inline = text.filter(|t| !t.trim().is_empty()).map(Arc::from),
                }
                let s = s.clone();
                drop(styles);
                self.0.doc.borrow_mut().set_style(n, s);
            }
            SVL_STYLE_OPACITY | SVL_STYLE_TRANSFORM | SVL_STYLE_CLIP | SVL_STYLE_INERT => {
                // kept until presto-core's effect and transform trees take JS channels (TR1)
                let mut vis = self.0.visuals.borrow_mut();
                visual_channel(vis.entry(n).or_default(), op, text)?;
            }
            other => return Err(format!("unknown style op {other}")),
        }
        Ok(0)
    }
}

/// Applies one TR1 channel to `v` (`None` text resets it), with egui's spellings.
pub fn visual_channel(v: &mut Visual, op: u32, text: Option<&str>) -> StrResult<()> {
    fn nums<const N: usize>(text: &str, what: &str) -> StrResult<[f32; N]> {
        let v: Vec<f32> = text.split_whitespace().map(str::parse::<f32>).collect::<Result<_, _>>().map_err(|_| format!("{what}: '{text}' is not {N} numbers"))?;
        v.try_into().map_err(|_| format!("{what}: '{text}' is not {N} numbers").into())
    }
    match op {
        SVL_STYLE_OPACITY => {
            v.opacity = match text {
                None => None,
                Some(t) => {
                    let t = t.trim();
                    let a = match t.strip_suffix('%') {
                        Some(p) => p.parse::<f32>().map(|x| x / 100.0),
                        None => t.parse::<f32>(),
                    }
                    .map_err(|_| format!("opacity: '{t}' is not a number"))?;
                    (a < 1.0).then_some(a.max(0.0))
                }
            }
        }
        SVL_STYLE_TRANSFORM => v.transform = text.map(|t| nums::<6>(t, "transform")).transpose()?,
        SVL_STYLE_CLIP => v.clip = text.map(|t| nums::<2>(t, "clip")).transpose()?,
        _ => v.inert = text.is_some(),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_parse_and_reset() {
        let mut v = Visual::default();
        visual_channel(&mut v, SVL_STYLE_OPACITY, Some("50%")).unwrap();
        assert_eq!(v.opacity, Some(0.5));
        visual_channel(&mut v, SVL_STYLE_OPACITY, Some("1")).unwrap();
        assert_eq!(v.opacity, None, "opaque is no channel");
        visual_channel(&mut v, SVL_STYLE_TRANSFORM, Some("1 0 0 1 10 0")).unwrap();
        assert_eq!(v.transform, Some([1.0, 0.0, 0.0, 1.0, 10.0, 0.0]));
        assert!(visual_channel(&mut v, SVL_STYLE_CLIP, Some("1")).is_err());
        visual_channel(&mut v, SVL_STYLE_INERT, Some("")).unwrap();
        assert!(v.inert);
        visual_channel(&mut v, SVL_STYLE_INERT, None).unwrap();
        assert!(!v.inert);
    }
}
