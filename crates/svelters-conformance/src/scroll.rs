//! WG3s: scroll samples and the motion metrics of `plans/milestones/WG3s-scroll-references.md` §3.
//!
//! A driver's `scroll_wheel` and `scroll_gesture` return a [`ScrollRecord`]: every scroll area's
//! offset sampled each frame (or every 8 ms) from the action's first input event until it settled,
//! and when the input ended. [`metrics`] turns it into the checkpoint's `motion`.

use crate::trace::{Motion, half_points};

/// The checkpoint pseudo-anchor that carries an action's [`metrics`].
pub const MOTION: &str = "motion";

/// The scroll fixture's largest offsets (content minus viewport, points): `s` 960 × 2400 in
/// 320 × 240, `t` 280 × 600 in 280 × 120.
pub fn max_offset(anchor: &str) -> [f64; 2] {
    match anchor {
        "t" => [0.0, 480.0],
        _ => [640.0, 2160.0],
    }
}

/// One sample: milliseconds from the action's first input event, the area, its offset.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub t_ms: f64,
    pub anchor: String,
    pub x: f64,
    pub y: f64,
}

/// What a scroll action recorded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScrollRecord {
    pub samples: Vec<Sample>,
    /// Each area's largest offset as the toolkit reports it (`(anchor, [x, y])`); [`max_offset`]
    /// when a driver gives none.
    pub max: Vec<(String, [f64; 2])>,
    /// When the last input event was sent (the wheel's last notch, the gesture's *ended*), ms.
    pub input_end_ms: f64,
}

impl ScrollRecord {
    /// The samples as JSON (`[[t_ms, anchor, x, y], …]`), for the trace directory.
    pub fn json(&self) -> String {
        let rows: Vec<String> = self
            .samples
            .iter()
            .map(|s| {
                format!(
                    "[{:.1},{},{:.2},{:.2}]",
                    s.t_ms,
                    serde_json::to_string(&s.anchor).unwrap(),
                    s.x,
                    s.y
                )
            })
            .collect();
        format!(
            "{{\"input_end_ms\":{:.1},\"samples\":[{}]}}",
            self.input_end_ms,
            rows.join(",")
        )
    }

    /// `(t_ms, position)` of one area on one axis.
    fn series(&self, anchor: &str, axis: usize) -> Vec<(f64, f64)> {
        self.samples
            .iter()
            .filter(|s| s.anchor == anchor)
            .map(|s| (s.t_ms, if axis == 0 { s.x } else { s.y }))
            .collect()
    }
}

/// The motion of an action on `target` along `axis` (0 x, 1 y). `notches`: a wheel action's
/// notch count (`None` for a gesture).
pub fn metrics(rec: &ScrollRecord, target: &str, axis: usize, notches: Option<u32>) -> Motion {
    let s = rec.series(target, axis);
    let (first, last) = match (s.first(), s.last()) {
        (Some(a), Some(b)) => (a.1, b.1),
        _ => return Motion::default(),
    };
    let distance = last - first;
    // peak velocity over 3-sample windows
    let mut peak = 0.0f64;
    for w in s.windows(3) {
        let dt = (w[2].0 - w[0].0) / 1000.0;
        if dt > 0.0 {
            peak = peak.max(((w[2].1 - w[0].1) / dt).abs());
        }
    }
    // the decay after the input ended: v = v0 e^(-t/τ), a least-squares line through ln |v|
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for w in s.windows(2) {
        if w[1].0 <= rec.input_end_ms {
            continue;
        }
        let dt = w[1].0 - w[0].0;
        let v = (w[1].1 - w[0].1).abs() / dt.max(1e-6);
        if dt > 0.0 && v > 1e-3 {
            pts.push((w[1].0, v.ln()));
        }
    }
    let tau_ms = (pts.len() >= 6)
        .then(|| {
            let n = pts.len() as f64;
            let (mx, my) = (
                pts.iter().map(|p| p.0).sum::<f64>() / n,
                pts.iter().map(|p| p.1).sum::<f64>() / n,
            );
            let sxy: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
            let sxx: f64 = pts.iter().map(|p| (p.0 - mx).powi(2)).sum();
            let slope = sxy / sxx.max(1e-9);
            (slope < 0.0).then(|| (-1.0 / slope).round() as i64)
        })
        .flatten();
    // the last change of more than a quarter point
    let mut last_change = rec.input_end_ms;
    for w in s.windows(2) {
        if (w[1].1 - w[0].1).abs() > 0.25 {
            last_change = last_change.max(w[1].0);
        }
    }
    let max = rec
        .max
        .iter()
        .find(|(a, _)| a == target)
        .map_or(max_offset(target), |m| m.1)[axis];
    let overshoot = s
        .iter()
        .map(|p| (-p.1).max(p.1 - max))
        .fold(0.0f64, f64::max);
    let chained = (target == "t").then(|| {
        let o = rec.series("s", axis);
        half_points(o.last().map_or(0.0, |p| p.1) - o.first().map_or(0.0, |p| p.1))
    });
    Motion {
        distance: half_points(distance),
        per_notch: notches
            .filter(|n| *n > 0)
            .map(|n| (distance.abs() / n as f64 * 10.0).round() as i64),
        peak_v: peak.round() as i64,
        tau_ms,
        settle_ms: (last_change - rec.input_end_ms).max(0.0).round() as i64,
        overshoot: half_points(overshoot),
        chained,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(points: &[(f64, f64)], end: f64) -> ScrollRecord {
        ScrollRecord {
            samples: points
                .iter()
                .map(|&(t, y)| Sample {
                    t_ms: t,
                    anchor: "s".into(),
                    x: 0.0,
                    y,
                })
                .collect(),
            input_end_ms: end,
            max: vec![],
        }
    }

    #[test]
    fn exponential_decay_gives_its_time_constant() {
        // v(t) = 2000 e^(-t/300) pt/s after the input ends at 100 ms, sampled every 16 ms
        let mut pts = vec![(0.0, 0.0), (100.0, 100.0)];
        let (mut y, mut t) = (100.0, 100.0);
        while t < 1500.0 {
            let v = 2000.0 * (-(t - 100.0) / 300.0f64).exp();
            t += 16.0;
            y += v * 0.016;
            pts.push((t, y));
        }
        let m = metrics(&rec(&pts, 100.0), "s", 1, None);
        let tau = m.tau_ms.expect("a decay");
        assert!((tau - 300).abs() <= 15, "tau {tau}");
        assert!(m.distance > 0 && m.overshoot == 0 && m.chained.is_none());
    }

    #[test]
    fn wheel_notches_and_overshoot() {
        let m = metrics(
            &rec(
                &[(0.0, 0.0), (16.0, 20.0), (32.0, 40.0), (300.0, 40.0)],
                0.0,
            ),
            "s",
            1,
            Some(1),
        );
        assert_eq!(m.distance, 80);
        assert_eq!(m.per_notch, Some(400));
        assert_eq!(m.tau_ms, None);
        let m = metrics(
            &rec(&[(0.0, 0.0), (16.0, -12.0), (200.0, 0.0)], 0.0),
            "s",
            1,
            None,
        );
        assert_eq!(m.overshoot, 24);
    }
}
