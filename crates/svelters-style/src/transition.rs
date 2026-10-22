//! [`Transitions`]: the CSS transitions store, sampled in Rust (ARCH-WIDGETS §3.4, DS2).
//!
//! The scene keeps one store keyed by node or `(node, part)`. After every restyle the scene calls
//! [`Transitions::restyled`] with the style before and after; each changed property whose
//! `transition` entry has a positive duration starts a transition from the value **painted now**
//! (a running transition's current value, else the old computed value) to the new computed value.
//! Paint reads [`Transitions::view`], which overlays the interpolated values at the frame's time;
//! [`Transitions::tick`] drops finished ones and says whether to request another frame.
//! Reduced motion (`prefers-reduced-motion`, TR F3) passes `instant: true`: nothing starts and
//! running transitions on that key end.
//!
//! Interpolated: colours (premultiplied sRGB), lengths, numbers, insets, font weights, shadow
//! lists (the shorter padded with transparent zero shadows) and gradients with the same stop
//! count. Everything else (keywords, strings) changes at once, or at the end of the delay when
//! the duration is 0 (`transition: background-color 0s 300ms`, R11).

use crate::computed::ComputedStyle;
use crate::shorthand::longhands;
use crate::values::{Color, ComputedValue, GradientStop, Length, LinearGradient, Shadow, TimingFunction};
use crate::view::StyleView;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::hash::Hash;

#[derive(Clone, Debug)]
struct Running {
    prop: usize,
    from: ComputedValue,
    to: ComputedValue,
    start: f64,
    duration: f32,
    delay: f32,
    timing: TimingFunction,
}

impl Running {
    /// The value at `now`, and whether the transition is over.
    fn sample(&self, now: f64) -> (ComputedValue, bool) {
        let elapsed = (now - self.start) as f32 - self.delay;
        // a zero duration with a delay switches at the end of the delay (R11's 300ms sash bar)
        let t = if self.duration > 0.0 { elapsed / self.duration } else if elapsed >= 0.0 { 1.0 } else { 0.0 };
        if t >= 1.0 {
            return (self.to.clone(), true);
        }
        if self.duration <= 0.0 {
            return (self.from.clone(), false);
        }
        let p = self.timing.apply(t.max(0.0));
        (lerp(&self.from, &self.to, p).unwrap_or_else(|| self.to.clone()), false)
    }
}

/// Running transitions by key (a node, or a node's part).
#[derive(Debug)]
pub struct Transitions<K> {
    running: HashMap<K, Vec<Running>>,
}

impl<K> Default for Transitions<K> {
    fn default() -> Self {
        Transitions { running: HashMap::new() }
    }
}

impl<K: Copy + Eq + Hash> Transitions<K> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a restyle of `key` from `before` to `after` at `now` (seconds, the frame's input
    /// time). Returns whether a transition started.
    pub fn restyled(&mut self, key: K, before: &ComputedStyle, after: &ComputedStyle, now: f64, instant: bool) -> bool {
        if instant {
            self.running.remove(&key);
            return false;
        }
        let table = after.table();
        let list = match after.get("transition") {
            Some(ComputedValue::Transitions(t)) => t.clone(),
            _ => Box::new([]),
        };
        let mut started = false;
        let changed = after.changed_since(before);
        if changed.is_empty() {
            return false;
        }
        let entry = self.running.entry(key).or_default();
        for i in changed {
            let name = table.props[i].name;
            if name == "transition" {
                continue;
            }
            let pos = entry.iter().position(|r| r.prop == i);
            // the last entry naming the property wins (CSS Transitions 1 §2)
            let spec = list.iter().rev().find(|t| &*t.property == "all" || &*t.property == name || longhands(&t.property).is_some_and(|l| l.contains(&name)));
            let (Some(spec), Some(to)) = (spec, after.get_index(i)) else {
                if let Some(k) = pos {
                    entry.remove(k);
                }
                continue;
            };
            if spec.duration <= 0.0 && spec.delay <= 0.0 {
                if let Some(k) = pos {
                    entry.remove(k);
                }
                continue;
            }
            let from = match pos {
                Some(k) => Some(entry[k].sample(now).0),
                None => before.get_index(i).cloned(),
            };
            let Some(from) = from.filter(|f| spec.duration <= 0.0 || lerp(f, to, 0.5).is_some()) else {
                if let Some(k) = pos {
                    entry.remove(k);
                }
                continue;
            };
            if let Some(k) = pos
                && entry[k].to == *to
            {
                continue; // already heading there
            }
            let r = Running { prop: i, from, to: to.clone(), start: now, duration: spec.duration, delay: spec.delay, timing: spec.timing };
            match pos {
                Some(k) => entry[k] = r,
                None => entry.push(r),
            }
            started = true;
        }
        if entry.is_empty() {
            self.running.remove(&key);
        }
        started
    }

    /// `style` (the key's current computed style) with the key's running transitions at `now`.
    pub fn view<'a>(&self, key: K, style: &'a ComputedStyle, now: f64) -> StyleView<'a> {
        let Some(list) = self.running.get(&key) else { return StyleView::new(style) };
        let over: SmallVec<[(usize, ComputedValue); 4]> = list.iter().map(|r| (r.prop, r.sample(now).0)).collect();
        StyleView::with(style, over)
    }

    pub fn is_running(&self, key: K) -> bool {
        self.running.contains_key(&key)
    }

    /// The keys with running transitions (TR2's `running_roots` maps them to roots).
    pub fn running_keys(&self) -> impl Iterator<Item = K> + '_ {
        self.running.keys().copied()
    }

    /// Drops the transitions that ended by `now`. Returns whether any still run (request a frame).
    pub fn tick(&mut self, now: f64) -> bool {
        self.running.retain(|_, list| {
            list.retain(|r| !r.sample(now).1);
            !list.is_empty()
        });
        !self.running.is_empty()
    }

    /// Forgets `key` (its node went away).
    pub fn remove(&mut self, key: K) {
        self.running.remove(&key);
    }

    /// Ends every transition (reduced motion switched on).
    pub fn clear(&mut self) {
        self.running.clear();
    }
}

fn mix(a: f32, b: f32, p: f32) -> f32 {
    a + (b - a) * p
}

fn mix_color(a: Color, b: Color, p: f32) -> Color {
    // premultiplied sRGB
    let al = mix(a.a, b.a, p);
    if al <= 0.0 {
        return Color::TRANSPARENT;
    }
    let ch = |x: f32, y: f32| mix(x * a.a, y * b.a, p) / al;
    Color { r: ch(a.r, b.r), g: ch(a.g, b.g), b: ch(a.b, b.b), a: al }
}

fn mix_len(a: Length, b: Length, p: f32) -> Length {
    Length { px: mix(a.px, b.px, p), percent: mix(a.percent, b.percent, p) }
}

/// The value `p` of the way from `a` to `b`, or `None` when the two don't interpolate.
fn lerp(a: &ComputedValue, b: &ComputedValue, p: f32) -> Option<ComputedValue> {
    use ComputedValue as V;
    Some(match (a, b) {
        (V::Color(x), V::Color(y)) => V::Color(mix_color(*x, *y, p)),
        (V::Length(x), V::Length(y)) => V::Length(mix_len(*x, *y, p)),
        (V::Number(x), V::Number(y)) => V::Number(mix(*x, *y, p)),
        (V::FontWeight(x), V::FontWeight(y)) => V::FontWeight(mix(*x as f32, *y as f32, p).round().clamp(1.0, 1000.0) as u16),
        (V::Insets(x), V::Insets(y)) => V::Insets(std::array::from_fn(|i| mix_len(x[i], y[i], p))),
        (V::Shadows(x), V::Shadows(y)) => {
            let n = x.len().max(y.len());
            let none = |s: &Shadow| Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 0.0, color: Some(Color::TRANSPARENT), inset: s.inset };
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let (sa, sb) = match (x.get(i), y.get(i)) {
                    (Some(a), Some(b)) => (*a, *b),
                    (Some(a), None) => (*a, none(a)),
                    (None, Some(b)) => (none(b), *b),
                    (None, None) => unreachable!(),
                };
                if sa.inset != sb.inset {
                    return None;
                }
                let color = match (sa.color, sb.color) {
                    (Some(c), Some(d)) => Some(mix_color(c, d, p)),
                    (c, d) => if p < 0.5 { c } else { d },
                };
                out.push(Shadow { x: mix(sa.x, sb.x, p), y: mix(sa.y, sb.y, p), blur: mix(sa.blur, sb.blur, p).max(0.0), spread: mix(sa.spread, sb.spread, p), color, inset: sa.inset });
            }
            V::Shadows(out.into())
        }
        (V::Image(Some(x)), V::Image(Some(y))) if x.stops.len() == y.stops.len() => V::Image(Some(Box::new(LinearGradient {
            angle: mix(x.angle, y.angle, p),
            stops: x.stops.iter().zip(y.stops.iter()).map(|(s, t)| GradientStop { color: mix_color(s.color, t.color, p), position: mix(s.position, t.position, p) }).collect(),
        }))),
        _ => return None,
    })
}
