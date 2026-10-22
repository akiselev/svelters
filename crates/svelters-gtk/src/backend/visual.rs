//! TR1 visual channels (ABI v2.8 `SVL_STYLE_TRANSFORM` / `CLIP` / `INERT`, and `bounds`): the
//! per-frame values of Svelte transitions (plans/milestones/TR1-transitions.md section 3).
//!
//! Opacity already goes through `gtk_widget_set_opacity` (`SVL_STYLE_OPACITY`, style.rs). Transform
//! and clip have no widget property in GTK 4, so they are CSS on a provider owned by the widget
//! alone (`gtk_style_context_add_provider`, deprecated since 4.10 but the only per-widget sink; the
//! display-wide inline provider of style.rs would be reloaded every frame). One short rule per
//! frame restyles and reallocates only that widget.
//!
//! * transform: `transform: translate() scale()` with `transform-origin` (GTK 4 CSS, paint only).
//! * clip: GTK cannot allocate a widget below its minimum size, so a clipped axis is approximated
//!   (decision D3): a negative margin gives the layout back the clipped part (siblings move) and a
//!   scale squashes the content into what is left, from the top/start edge.
//! * inert: `can-target` and `can-focus` off (the runtime also drops the subtree's events).
use super::{Inner, Res};
use gtk::prelude::*;

/// What the channels changed on a widget, kept on the widget itself (so it goes with it).
#[derive(Default)]
struct Visual {
    provider: Option<gtk::CssProvider>,
    /// tx, ty, sx, sy, ox, oy
    transform: Option<[f64; 6]>,
    /// w, h (px; < 0 = that axis unclipped)
    clip: Option<[f64; 2]>,
    /// can-target, can-focus before the widget was made inert
    inert: Option<(bool, bool)>,
}

const KEY: &str = "svelters-visual";

fn visual(w: &gtk::Widget) -> &mut Visual {
    // SAFETY: the data is only ever stored under KEY with this type, and only touched on the UI thread.
    unsafe {
        if w.data::<Visual>(KEY).is_none() {
            w.set_data(KEY, Visual::default());
        }
        &mut *w.data::<Visual>(KEY).unwrap().as_ptr()
    }
}

fn nums<const N: usize>(text: &str, what: &str) -> Res<[f64; N]> {
    let v: Vec<f64> = text.split_whitespace().map(|t| t.parse::<f64>()).collect::<Result<_, _>>().map_err(|_| format!("{what}: '{text}' is not {N} numbers"))?;
    v.try_into().map_err(|_| format!("{what}: '{text}' is not {N} numbers"))
}

/// The natural size of `w` (height for its current or natural width).
fn natural(w: &gtk::Widget) -> (f64, f64) {
    let (_, nat_w, _, _) = w.measure(gtk::Orientation::Horizontal, -1);
    let width = if w.width() > 0 { w.width() } else { nat_w };
    let (_, nat_h, _, _) = w.measure(gtk::Orientation::Vertical, width);
    (nat_w as f64, nat_h as f64)
}

fn css_num(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    if r == r.trunc() { format!("{}", r as i64) } else { format!("{r}") }
}

impl Inner {
    pub(crate) fn visual_op(&self, op: u32, h: u64, text: Option<&str>) -> Res<()> {
        let (w, _) = self.widget_of(h)?;
        let v = visual(&w);
        match op {
            svelters_abi::SVL_STYLE_TRANSFORM => v.transform = text.map(|t| nums::<6>(t, "transform")).transpose()?,
            svelters_abi::SVL_STYLE_CLIP => v.clip = text.map(|t| nums::<2>(t, "clip")).transpose()?,
            svelters_abi::SVL_STYLE_INERT => {
                if text.is_some() {
                    if v.inert.is_none() {
                        v.inert = Some((w.can_target(), w.can_focus()));
                        w.set_can_target(false);
                        w.set_can_focus(false);
                    }
                } else if let Some((t, f)) = v.inert.take() {
                    w.set_can_target(t);
                    w.set_can_focus(f);
                }
                return Ok(());
            }
            _ => unreachable!(),
        }
        self.apply_visual(&w);
        Ok(())
    }

    /// Writes the widget's transform + clip rule into its own provider.
    fn apply_visual(&self, w: &gtk::Widget) {
        let v = visual(w);
        let mut decls = String::new();
        let (tx, ty, mut sx, mut sy, mut ox, mut oy) = match v.transform {
            Some([tx, ty, sx, sy, ox, oy]) => (tx, ty, sx, sy, ox, oy),
            None => (0.0, 0.0, 1.0, 1.0, 0.5, 0.5),
        };
        if let Some([cw, ch]) = v.clip {
            let (nw, nh) = natural(w);
            if ch >= 0.0 && nh > 0.0 && ch < nh {
                decls.push_str(&format!("margin-bottom: -{}px; ", css_num(nh - ch)));
                sy *= ch / nh;
                if v.transform.is_none() {
                    oy = 0.0;
                }
            }
            if cw >= 0.0 && nw > 0.0 && cw < nw {
                decls.push_str(&format!("margin-right: -{}px; ", css_num(nw - cw)));
                sx *= cw / nw;
                if v.transform.is_none() {
                    ox = 0.0;
                }
            }
        }
        if v.transform.is_some() || sx != 1.0 || sy != 1.0 {
            decls.push_str(&format!(
                "transform: translate({}px, {}px) scale({}, {}); transform-origin: {}% {}%;",
                css_num(tx),
                css_num(ty),
                css_num(sx),
                css_num(sy),
                css_num(ox * 100.0),
                css_num(oy * 100.0)
            ));
        }
        #[allow(deprecated)]
        let ctx = w.style_context();
        if decls.is_empty() {
            if let Some(p) = v.provider.take() {
                #[allow(deprecated)]
                ctx.remove_provider(&p);
            }
            return;
        }
        let p = match &v.provider {
            Some(p) => p.clone(),
            None => {
                let p = gtk::CssProvider::new();
                #[allow(deprecated)]
                ctx.add_provider(&p, gtk::STYLE_PROVIDER_PRIORITY_USER + 1);
                v.provider = Some(p.clone());
                p
            }
        };
        p.load_from_data(&format!("* {{ {decls} }}"));
    }

    /// ABI v2.8 `bounds`: the widget's rect in its root's coordinates; natural size at 0, 0 when it
    /// has not been allocated yet.
    pub(crate) fn bounds_of(&self, h: u64) -> Res<[f64; 4]> {
        let (w, _) = self.widget_of(h)?;
        if w.width() > 0 || w.height() > 0 {
            if let Some(root) = w.root() {
                if let Some(r) = w.compute_bounds(root.upcast_ref::<gtk::Widget>()) {
                    return Ok([r.x() as f64, r.y() as f64, r.width() as f64, r.height() as f64]);
                }
            }
        }
        let (nw, nh) = natural(&w);
        Ok([0.0, 0.0, nw, nh])
    }
}
